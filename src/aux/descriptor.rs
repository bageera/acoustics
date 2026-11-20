//! The fixed 38-byte auxiliary stream descriptor (docs/neuro-overlay-spec.md
//! §3): fail-closed parse and write, every field validated.

use super::limits::*;

/// One auxiliary stream's descriptor (38 bytes, little-endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamDescriptor {
    /// 0 = sine, 1 = sine with theta-AM
    pub model: u8,
    /// bit0 = enabled; unknown set bits reject
    pub flags: u8,
    /// millihertz (500..=20_000_000)
    pub carrier_mhz: u32,
    /// Q16 turns
    pub carrier_phase_q16: u16,
    /// Q15 full-scale fraction; the render's PEAK is amplitude×(1+depth)
    pub amplitude_q15: u16,
    /// millihertz (model 1 only, 100..=12_000; model 0 MUST be 0)
    pub mod_mhz: u32,
    /// Q16 (0..=65535 ⇒ depth 0..=0.99998; model 0 MUST be 0)
    pub mod_depth_q16: u16,
    /// Q16 turns; model 0 MUST be 0
    pub mod_phase_q16: u16,
    pub start_sample: u64,
    /// 0 = until the primary ends (still bounded by MAX_DURATION_SAMPLES)
    pub duration_samples: u64,
    /// bit0 = channel 0 (L), bit1 = channel 1 (R); 0b00 rejects
    pub routing: u8,
    /// 0 = none; else the 1-based index of the MASTER stream whose theta
    /// phase drives this stream's amplitude modulation (forward-only)
    pub coupling_ref: u8,
}

pub const STREAM_LEN: usize = 38;

const ERR_AUX_MODEL: &str = "aux descriptor: unknown model id";
const ERR_AUX_FLAG: &str = "aux descriptor: unknown flag bits";
const ERR_AUX_FREQ: &str = "aux descriptor: carrier frequency out of bounds";
const ERR_AUX_MODF: &str = "aux descriptor: modulation frequency out of bounds";
const ERR_AUX_MOD0: &str = "aux descriptor: model 0 must carry no modulation";
const ERR_AUX_ROUTE: &str = "aux descriptor: invalid routing mask";
const ERR_AUX_RESERVED: &str = "aux descriptor: nonzero reserved bytes";
const ERR_AUX_TRUNC: &str = "aux descriptor: truncated";
const ERR_AUX_COUPLE: &str = "aux descriptor: invalid coupling reference";

impl StreamDescriptor {
    /// Fail-closed parse: any unknown or out-of-bounds byte is an error.
    pub fn parse(buf: &[u8]) -> Result<(Self, usize), String> {
        if buf.len() < STREAM_LEN {
            return Err(ERR_AUX_TRUNC.to_string());
        }
        let model = buf[0];
        if model > 1 {
            return Err(format!("{ERR_AUX_MODEL} {}", model));
        }
        let flags = buf[1];
        if flags & !0x01 != 0 {
            return Err(format!("{ERR_AUX_FLAG} {flags:#x}"));
        }
        let rd = |o: usize| -> u32 { u32::from_le_bytes(buf[o..o + 4].try_into().unwrap()) };
        let carrier_mhz = rd(2);
        if !(CARRIER_MHZ_MIN..=CARRIER_MHZ_MAX).contains(&carrier_mhz) {
            return Err(format!("{ERR_AUX_FREQ} {carrier_mhz} mHz"));
        }
        let carrier_phase_q16 = u16::from_le_bytes(buf[6..8].try_into().unwrap());
        let amplitude_q15 = u16::from_le_bytes(buf[8..10].try_into().unwrap());
        let mod_mhz = rd(10);
        let mod_depth_q16 = u16::from_le_bytes(buf[14..16].try_into().unwrap());
        let mod_phase_q16 = u16::from_le_bytes(buf[16..18].try_into().unwrap());
        if model == 0 && (mod_mhz != 0 || mod_depth_q16 != 0 || mod_phase_q16 != 0) {
            return Err(ERR_AUX_MOD0.to_string());
        }
        if model == 1 {
            if !(MOD_MHZ_MIN..=MOD_MHZ_MAX).contains(&mod_mhz) {
                return Err(format!("{ERR_AUX_MODF} {mod_mhz} mHz"));
            }
            if mod_depth_q16 == 0 {
                return Err(format!("{ERR_AUX_MODF} (0 depth on an AM model)"));
            }
        }
        let rd64 = |o: usize| -> u64 { u64::from_le_bytes(buf[o..o + 8].try_into().unwrap()) };
        let start_sample = rd64(18);
        let duration_samples = rd64(26);
        let routing = buf[34];
        if routing == 0 || routing & !0x03 != 0 {
            return Err(format!("{ERR_AUX_ROUTE} {routing:#04x}"));
        }
        let coupling_ref = buf[35];
        // (the coupling's referent existence is checked at the FILE level)
        if coupling_ref > MAX_STREAMS as u8 {
            return Err(format!("{ERR_AUX_COUPLE} {coupling_ref}"));
        }
        let reserved = &buf[36..38];
        if reserved.iter().any(|&b| b != 0) {
            return Err(ERR_AUX_RESERVED.to_string());
        }
        let d = Self {
            model,
            flags,
            carrier_mhz,
            carrier_phase_q16,
            amplitude_q15,
            mod_mhz,
            mod_depth_q16,
            mod_phase_q16,
            start_sample,
            duration_samples,
            routing,
            coupling_ref,
        };
        Ok((d, STREAM_LEN))
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.push(self.model);
        out.push(self.flags);
        out.extend_from_slice(&self.carrier_mhz.to_le_bytes());
        out.extend_from_slice(&self.carrier_phase_q16.to_le_bytes());
        out.extend_from_slice(&self.amplitude_q15.to_le_bytes());
        out.extend_from_slice(&self.mod_mhz.to_le_bytes());
        out.extend_from_slice(&self.mod_depth_q16.to_le_bytes());
        out.extend_from_slice(&self.mod_phase_q16.to_le_bytes());
        out.extend_from_slice(&self.start_sample.to_le_bytes());
        out.extend_from_slice(&self.duration_samples.to_le_bytes());
        out.push(self.routing);
        out.push(self.coupling_ref);
        out.extend_from_slice(&[0u8, 0u8]);
    }

    /// The effective render end (bounded by MAX_DURATION_SAMPLES and 0=∞).
    pub fn render_end(&self, primary_len: u64) -> u64 {
        if self.duration_samples == 0 {
            return primary_len.min(MAX_DURATION_SAMPLES);
        }
        self.duration_samples.min(MAX_DURATION_SAMPLES)
    }
}
