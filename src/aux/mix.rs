//! The N25 derived-playback mixer: widened-arithmetic, block-size-
//! independent headroom policy. The policy is computed ONCE from the
//! declared per-stream peaks (P_max = A×(1+d), combined by summation), so
//! the no-headroom decision can never depend on the render block size.

use super::render::render_window;
use super::wire::AuxSpec;

/// The fixed mix ceiling (0.98 full scale — the N21/N22 declared policy).
pub const MIX_CEILING: i64 = 32_112;

/// The applied-mix report (headroom accounting is never silent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixReport {
    /// Σ P_max = Σ A_i×(1+d_i) (+ a small table-quantization margin)
    pub combined_peak: i64,
    /// max |primary| over the mixed span
    pub primary_peak: i64,
    /// the applied integer overlay scale in Q16 (65536 = unchanged,
    /// 0 = omitted entirely)
    pub scale_q16: u64,
    /// any sample clamped during the mix (0 by policy; nonzero = a BUG
    /// signal, reported by the caller)
    pub clamped: u64,
}

/// Per-stream safe peak P_max in PCM units, +4 for the Q14 table's
/// quantization wiggle (the N24 headroom test's measured envelope).
pub fn stream_peak(d: &super::descriptor::StreamDescriptor) -> i64 {
    let amp = d.amplitude_q15 as i64;
    let env_max = (1 << 15) + (((d.mod_depth_q16 as i64) * (1 << 14)) >> 15);
    let base = ((amp << 14) * env_max) >> 29;
    base + 4
}

/// Mix the sidecar's overlay into the primary PCM channels IN PLACE
/// (the primary Vec is the caller's decoded copy — never the user's file).
///
/// Policy (pre-declared, N21/N22): the overlay amplitude is scaled DOWN
/// uniformly (or omitted at scale 0) when Σ P_max does not fit under the
/// ceiling given the primary's own peak; gain is never escalated; mixing
/// is i64-widened and every clamp is counted.
pub fn mix_into_primary(
    spec: &AuxSpec,
    rate: u32,
    primary_count: u64,
    chans: &mut [Vec<i16>],
) -> Result<MixReport, String> {
    let channels = chans.len();
    if channels == 0 || channels > 2 {
        return Err("aux mix: 1 or 2 channels only".to_string());
    }
    let count = chans[0].len();
    let primary_peak = chans
        .iter()
        .flat_map(|c| c.iter())
        .map(|&v| (v as i64).abs())
        .max()
        .unwrap_or(0);
    let combined_peak: i64 = spec.streams.iter().map(stream_peak).sum();
    let budget = MIX_CEILING - primary_peak;
    let scale_q16: u64 = if combined_peak <= 0 {
        0
    } else if combined_peak <= budget {
        65536
    } else if budget <= 0 {
        0
    } else {
        ((budget as u128) * 65536 / (combined_peak as u128)) as u64
    };
    let mut report = MixReport {
        combined_peak,
        primary_peak,
        scale_q16,
        clamped: 0,
    };
    if scale_q16 == 0 {
        // the documented omission policy: render nothing rather than clip
        return Ok(report);
    }
    // render the overlay into temp channels, then add with the scale
    // pre-scale the DESCRIPTOR amplitudes (the scale acts on each stream's
    // own peak BEFORE any clamping — a stream whose P_max exceeds i16 on its
    // own must be shrunk at the source, not saved by a post-clamp)
    let mut scaled_spec = spec.clone();
    for s in &mut scaled_spec.streams {
        s.amplitude_q15 = (((s.amplitude_q15 as u64) * scale_q16) >> 16) as u16;
    }
    let mut ov = vec![vec![0i16; count]; channels];
    report.clamped += render_window(&scaled_spec, rate, primary_count, 0, count, &mut ov)?.clamped;
    for (ci, chan) in chans.iter_mut().enumerate().take(channels) {
        for (i, slot) in chan.iter_mut().enumerate() {
            let raw = (*slot as i64) + ov[ci][i] as i64;
            let v = raw.clamp(i16::MIN as i64, i16::MAX as i64);
            if v != raw {
                report.clamped += 1;
            }
            *slot = v as i16;
        }
    }
    Ok(report)
}
