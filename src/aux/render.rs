//! The normative auxiliary-stream renderer (docs/neuro-overlay-spec.md §4):
//! pure-integer, platform-independent, index-addressable (bit-exact random
//! access), block-independent, incrementally renderable in bounded windows.

use super::descriptor::StreamDescriptor;
use super::wire::AuxSpec;

const PHASE_BITS: u32 = 48;
const PHASE_MASK: u64 = (1u64 << PHASE_BITS) - 1;
const TABLE_N: usize = 16384;

// The Q14 sine table is INCLUDED at item scope from the generated file
// (scripts/neuro/aux_reference.py emits it with its own `static` declaration).
include!("test_vectors_table.rs");

/// The normative half-up integer rounding of freq_mHz × 2^48 / (rate×1000).
pub fn step_u48(freq_mhz: u32, rate: u32) -> u64 {
    let num = (freq_mhz as u128) << PHASE_BITS;
    let den = rate as u128 * 1000;
    ((num + den / 2) / den) as u64
}

/// phase(i) for a Q16 phase0 in 48-bit fixed point.
pub fn phase_at(phase0_q16: u16, freq_mhz: u32, rate: u32, i: u64) -> u64 {
    let p0 = ((phase0_q16 as u64) << 32) & PHASE_MASK;
    let step = step_u48(freq_mhz, rate);
    ((p0 as u128 + (step as u128) * (i as u128)) as u64) & PHASE_MASK
}

/// The normative Q14 waveform evaluation (integer linear interpolation).
pub fn eval_q14(phase48: u64) -> i32 {
    let idx = (((phase48 as u128) * TABLE_N as u128) >> PHASE_BITS) as usize;
    let frac = ((((phase48 & PHASE_MASK) as u128) * TABLE_N as u128) >> PHASE_BITS) as u64;
    let a = AUX_TABLE14[idx] as i32;
    let b = AUX_TABLE14[(idx + 1) % TABLE_N] as i32;
    a + (((b - a) * frac as i32) >> 14)
}

/// One stream's rendered PCM value at absolute sample index `i`
/// (pre-clamp i64). `master_theta_phase48` overrides the stream's own
/// modulation phase when the stream is a coupled slave.
pub fn stream_sample(
    d: &StreamDescriptor,
    rate: u32,
    i: u64,
    master_theta_phase48: Option<u64>,
) -> i64 {
    let v = eval_q14(phase_at(d.carrier_phase_q16, d.carrier_mhz, rate, i));
    let env_q15: i64 = if d.mod_depth_q16 == 0 {
        1 << 15
    } else {
        let theta_q14 = match master_theta_phase48 {
            Some(ph) => eval_q14(ph),
            None => eval_q14(phase_at(d.mod_phase_q16, d.mod_mhz, rate, i)),
        };
        (1 << 15) + (((d.mod_depth_q16 as i64) * theta_q14 as i64) >> 15)
    };
    ((d.amplitude_q15 as i64) * v as i64 * env_q15) >> 29
}

/// Clamp with an audit: the count of clamped samples is REPORTED, never
/// silent (the spec's output-range contract).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClampAudit {
    pub clamped: u64,
}

/// Render the auxiliary signals of `spec` into per-channel PCM buffers for
/// the window [start, start+count). Buffers are the caller's (bounded
/// memory: no render larger than the window is materialized).
/// `primary_count` bounds the streams' windows (spec §5).
pub fn render_window(
    spec: &AuxSpec,
    rate: u32,
    primary_count: u64,
    start: u64,
    count: usize,
    chans: &mut [Vec<i16>],
) -> Result<ClampAudit, String> {
    let channels = chans.len();
    if channels == 0 || channels > 2 {
        return Err("acaux render: 1 or 2 channels only".to_string());
    }
    if (rate as u64) == 0 {
        return Err("acaux render: invalid rate".to_string());
    }
    // N26-A: the caller's buffers must cover the whole window — a hostile
    // (or accidental) oversized request REJECTS instead of slicing panicking
    for c in chans.iter() {
        if c.len() < count {
            return Err(format!(
                "acaux render: channel buffer {} < the requested window {count}",
                c.len()
            ));
        }
    }
    let mut audit = ClampAudit::default();
    let (first, mut second): (&mut [i16], Option<&mut [i16]>) = if chans.len() > 1 {
        let (a, b) = chans.split_at_mut(1);
        (&mut a[0][..count], Some(&mut b[0][..count]))
    } else {
        let a: &mut [i16] = &mut chans[0];
        (&mut a[..count], None)
    };
    for (i, slot0) in first.iter_mut().enumerate() {
        let idx = start + i as u64;
        let mut acc = [0i64; 2];
        for d in &spec.streams {
            if d.flags & 0x01 == 0 {
                continue; // disabled
            }
            let end = d.start_sample.saturating_add(d.render_end(primary_count));
            if idx < d.start_sample || idx >= end {
                continue;
            }
            let master_ph = if d.coupling_ref != 0 {
                let m = &spec.streams[d.coupling_ref as usize - 1];
                Some(phase_at(m.mod_phase_q16, m.mod_mhz, rate, idx))
            } else {
                None
            };
            let raw = stream_sample(d, rate, idx, master_ph);
            let v16 = raw.clamp(i16::MIN as i64, i16::MAX as i64);
            if v16 != raw {
                audit.clamped += 1;
            }
            for (c, slot) in acc.iter_mut().enumerate().take(channels) {
                if d.routing & (1 << c) != 0 {
                    *slot += v16;
                }
            }
        }
        for (c, &accv) in acc.iter().enumerate() {
            let v = accv.clamp(i16::MIN as i64, i16::MAX as i64);
            if v != accv {
                audit.clamped += 1;
            }
            match c {
                0 => *slot0 = v as i16,
                1 => {
                    if let Some(s) = second.as_mut() {
                        s[i] = v as i16;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(audit)
}
