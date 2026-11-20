//! The `.acaux` sidecar container (docs/neuro-overlay-spec.md §2):
//! fail-closed parse/write with the sha-256 content binding, the file
//! length and the CRC-16/CCITT-FALSE integrity trailer.

use super::descriptor::{StreamDescriptor, STREAM_LEN};
use super::limits::*;

pub const HEADER_LEN: usize = 16;
pub const TRAILER_LEN: usize = 6;

/// The full parsed sidecar: validated streams + the binding hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxSpec {
    pub streams: Vec<StreamDescriptor>,
    /// sha256(primary)[0..8] — checked by the caller against the actual file
    pub binding: [u8; 8],
}

const ERR_MAGIC: &str = "acaux: bad magic";
const ERR_VER: &str = "acaux: unknown version";
const ERR_FLAGS: &str = "acaux: unknown flag bits";
const ERR_COUNT: &str = "acaux: stream count out of bounds";
const ERR_RESERVED: &str = "acaux: nonzero reserved byte";
const ERR_LEN: &str = "acaux: declared length mismatch";
const ERR_TRAIL: &str = "acaux: trailing bytes after the declared end";
const ERR_CRC: &str = "acaux: crc16 mismatch";
const ERR_COUPLE: &str = "acaux: coupling reference to an unknown stream";
const ERR_SELF: &str = "acaux: self-referencing or non-forward coupling";

/// CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF, no reflection).
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

impl AuxSpec {
    /// Fail-closed parse of a complete `.acaux` file.
    /// `expected_binding` (when Some) is cross-checked against the file.
    pub fn parse(data: &[u8], expected_binding: Option<&[u8; 8]>) -> Result<Self, String> {
        if data.len() < HEADER_LEN + TRAILER_LEN {
            return Err("acaux: truncated header".to_string());
        }
        if &data[0..4] != b"ACAU" {
            return Err(ERR_MAGIC.to_string());
        }
        if data[4] != 1 {
            return Err(format!("{ERR_VER} {}", data[4]));
        }
        if data[5] != 0 {
            return Err(format!("{ERR_FLAGS} {:#x}", data[5]));
        }
        let mut binding = [0u8; 8];
        binding.copy_from_slice(&data[6..14]);
        if let Some(want) = expected_binding {
            if want != &binding {
                return Err("acaux: primary binding mismatch".to_string());
            }
        }
        let count = data[14] as usize;
        if count == 0 || count > MAX_STREAMS {
            return Err(format!("{ERR_COUNT} {count}"));
        }
        if data[15] != 0 {
            return Err(ERR_RESERVED.to_string());
        }
        let streams_end = HEADER_LEN + count * STREAM_LEN;
        let declared_end = streams_end + TRAILER_LEN;
        if data.len() < declared_end {
            return Err("acaux: truncated streams".to_string());
        }
        let (declared_len, crc_stored) = {
            let o = streams_end;
            (
                u32::from_le_bytes(data[o..o + 4].try_into().unwrap()),
                u16::from_le_bytes(data[o + 4..o + 6].try_into().unwrap()),
            )
        };
        if declared_len as usize != data.len() {
            return Err(format!("{ERR_LEN} {declared_len} vs {}", data.len()));
        }
        if data.len() > declared_end {
            return Err(format!("{ERR_TRAIL} {}", data.len() - declared_end));
        }
        // the crc covers [0, streams_end)
        let crc_ok = crc16_ccitt_false(&data[..streams_end]) == crc_stored;
        if !crc_ok {
            return Err(format!("{ERR_CRC} {crc_stored:#06x}"));
        }
        let mut streams = Vec::with_capacity(count);
        let mut pos = HEADER_LEN;
        for _ in 0..count {
            let (d, n) = StreamDescriptor::parse(&data[pos..])?;
            pos += n;
            streams.push(d);
        }
        // coupling integrity: forward-only references to existing streams
        for (i, s) in streams.iter().enumerate() {
            if s.coupling_ref != 0 {
                let id = s.coupling_ref as usize;
                if id > count {
                    return Err(format!("{ERR_COUPLE} {id} (count {count})"));
                }
                if id == i + 1 || id > i {
                    return Err(format!("{ERR_SELF} stream {} -> {id}", i + 1));
                }
                // the master must be an AM model with a real theta
                if streams[id - 1].model != 1 {
                    return Err(format!("{ERR_COUPLE} {id} is not a modulation master"));
                }
            }
        }
        Ok(Self { streams, binding })
    }

    /// Serialize the complete file (header + streams + length + crc).
    pub fn write(&self) -> Vec<u8> {
        let mut out =
            Vec::with_capacity(HEADER_LEN + self.streams.len() * STREAM_LEN + TRAILER_LEN);
        out.extend_from_slice(b"ACAU");
        out.push(1); // version
        out.push(0); // flags
        out.extend_from_slice(&self.binding);
        out.push(self.streams.len() as u8);
        out.push(0); // reserved
        for s in &self.streams {
            s.write(&mut out);
        }
        let crc = crc16_ccitt_false(&out[..HEADER_LEN + self.streams.len() * STREAM_LEN]);
        out.extend_from_slice(&(out.len() as u32 + 6).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    /// Validate against the PRIMARY dimensions (rate/count bounds).
    /// Every stream's window must fit inside the primary.
    pub fn validate_against_primary(&self, rate: u32, primary_count: u64) -> Result<(), String> {
        if !(RATE_MIN..=RATE_MAX).contains(&rate) {
            return Err(format!("acaux: primary rate {} out of bounds", rate));
        }
        for (i, s) in self.streams.iter().enumerate() {
            // spec §3: start_sample in 0..=primary_count — unconditionally
            // (a duration-0 stream still may not start past the primary)
            if s.start_sample > primary_count {
                return Err(format!("acaux: stream {} starts past the primary", i + 1));
            }
            if s.duration_samples > MAX_DURATION_SAMPLES {
                return Err(format!(
                    "acaux: stream {} exceeds the duration limit",
                    i + 1
                ));
            }
        }
        Ok(())
    }
}
