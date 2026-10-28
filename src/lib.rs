// ACOUSTICS Engine v0.1 — reversible audio-model codec baseline.
// Models: RAW (exact reference), D1 (first-order differences), D2 (second-order),
// PERIOD (differences against earlier samples at candidate delays).
// Model selection = smallest complete serialized payload. Decode = exact.
use std::convert::TryFrom;

pub const MAGIC: &[u8; 8] = b"ACUSTCS1";
pub const VERSION: u8 = 1;
pub const MODEL_RAW: u8 = 0;
pub const MODEL_D1: u8 = 1;
pub const MODEL_D2: u8 = 2;
pub const MODEL_PERIOD: u8 = 3;
const VARINT_MAX_BYTES: usize = 10;

const ERR_BAD_MAGIC: &str = "bad magic (not an ACOUSTICS payload)";
const ERR_TRUNCATED: &str = "truncated payload";
const ERR_EMPTY: &str = "no samples";
const ERR_RANGE: &str = "decoded sample out of i16 range";

// ---------- integer coding ----------

fn zigzag_encode(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}

fn zigzag_decode(x: u64) -> i64 {
    ((x >> 1) as i64) ^ -((x & 1) as i64)
}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

fn read_varint(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut result: u64 = 0;
    let mut bytes = 0usize;
    loop {
        let b = *data
            .get(*pos)
            .ok_or_else(|| format!("{ERR_TRUNCATED} (varint overruns payload end)"))?;
        *pos += 1;
        result |= ((b & 0x7F) as u64) << (bytes * 7);
        if b & 0x80 == 0 {
            return Ok(result);
        }
        bytes += 1;
        if bytes >= VARINT_MAX_BYTES {
            return Err("varint too long for u64".to_string());
        }
    }
}

fn varint_len(v: u64) -> usize {
    if v == 0 {
        return 1;
    }
    let bits = 64 - v.leading_zeros();
    (bits as usize).div_ceil(7)
}

/// Fixed header: magic(8) + version(1) + kind(1) + rate(4) + count varint.
fn header_len(count: usize) -> usize {
    14 + varint_len(count as u64)
}

fn i16_from_i64(v: i64) -> Result<i16, String> {
    i16::try_from(v).map_err(|_| ERR_RANGE.into())
}

// ---------- WAV I/O (mono 16-bit PCM only) ----------

/// Parse a mono 16-bit PCM WAV (RIFF). Metadata is ignored.
pub fn parse_wav(data: &[u8]) -> Result<(u32, Vec<i16>), String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut cursor = 12;
    let mut rate: Option<u32> = None;
    while cursor + 8 <= data.len() {
        let id: [u8; 4] = data[cursor..cursor + 4].try_into().unwrap();
        let size = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let body_end = (cursor + 8 + size).min(data.len());
        let body = &data[cursor + 8..body_end];
        if &id == b"fmt " {
            if size < 16 {
                return Err("truncated fmt chunk".into());
            }
            let format = u16::from_le_bytes(body[0..2].try_into().unwrap());
            let channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
            let sample_rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            let bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
            if format != 1 || channels != 1 || bits != 16 {
                return Err("only mono 16-bit PCM WAV supported".into());
            }
            if sample_rate == 0 {
                return Err("invalid sample rate".into());
            }
            rate = Some(sample_rate);
        } else if &id == b"data" {
            if body.len() < 2 {
                return Err("empty data chunk".into());
            }
            if !body.len().is_multiple_of(2) {
                return Err("odd-sized data chunk".into());
            }
            let samples: Vec<i16> = body
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes(c.try_into().unwrap()))
                .collect();
            return match rate {
                Some(r) if !samples.is_empty() => Ok((r, samples)),
                Some(_) => Err(ERR_EMPTY.into()),
                None => Err("data chunk before fmt chunk".into()),
            };
        }
        cursor += 8 + size + (size & 1);
    }
    // ponytail: trailing chunks after data are never searched — real WAVs never need it
    Err("missing data chunk".into())
}

/// Minimal 44-byte mono 16-bit PCM WAV writer.
pub fn write_wav(sample_rate: u32, samples: &[i16]) -> Result<Vec<u8>, String> {
    if sample_rate == 0 {
        return Err("invalid sample rate".into());
    }
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(out)
}

// ---------- models ----------

/// Selected model: kind + PERIOD lag (0 otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub kind: u8,
    pub lag: usize,
}

fn predictor_residuals(samples: &[i16], kind: u8) -> Vec<i64> {
    match kind {
        MODEL_D1 => {
            let mut r = Vec::with_capacity(samples.len());
            r.push(samples[0] as i64);
            for w in samples.windows(2) {
                r.push(w[1] as i64 - w[0] as i64);
            }
            r
        }
        MODEL_D2 => {
            let mut r = Vec::with_capacity(samples.len());
            r.push(samples[0] as i64);
            if samples.len() > 1 {
                r.push(samples[1] as i64 - samples[0] as i64);
            }
            for w in samples.windows(3) {
                r.push((w[2] as i64 - w[1] as i64) - (w[1] as i64 - w[0] as i64));
            }
            r
        }
        _ => samples.iter().map(|&s| s as i64).collect(),
    }
}

/// PERIOD residuals + selection masks. Bit i of masks[(i-lag)/32] says the lag
/// delta is coded at sample i; zero residuals are never coded (mask bit clear).
fn period_plan(samples: &[i16], lag: usize) -> Option<(Vec<i64>, Vec<u32>)> {
    if lag == 0 || lag >= samples.len() {
        return None;
    }
    let tail = samples.len() - lag;
    let mut residuals = Vec::with_capacity(samples.len());
    for &s in &samples[..lag] {
        residuals.push(s as i64);
    }
    let mut masks = vec![0u32; tail.div_ceil(32)];
    for i in lag..samples.len() {
        let residual = samples[i] as i64 - samples[i - lag] as i64;
        if residual != 0 {
            masks[(i - lag) / 32] |= 1 << ((i - lag) % 32);
        }
        residuals.push(residual);
    }
    Some((residuals, masks))
}

fn varint_sum<'a>(it: impl Iterator<Item = &'a i64>) -> usize {
    it.map(|&r| varint_len(zigzag_encode(r))).sum()
}

/// Exact serialized size for a PERIOD payload (selection metric; no Vec churn).
fn period_payload_len(samples: &[i16], lag: usize) -> Option<usize> {
    let (residuals, masks) = period_plan(samples, lag)?;
    let head_bytes = samples[..lag]
        .iter()
        .map(|&s| varint_len(zigzag_encode(s as i64)))
        .sum::<usize>();
    // only nonzero tail residuals are actually coded (mask bit set)
    let tail_bytes = varint_sum(residuals[lag..].iter().filter(|&&r| r != 0));
    Some(
        header_len(samples.len())
            + varint_len(lag as u64)
            + masks.len() * 4
            + head_bytes
            + tail_bytes,
    )
}

/// Exact serialized size of a complete payload for `kind` (selection metric).
fn payload_len(kind: u8, lag: usize, samples: &[i16]) -> Option<usize> {
    if samples.is_empty() {
        return None;
    }
    Some(match kind {
        MODEL_PERIOD => period_payload_len(samples, lag)?,
        _ => header_len(samples.len()) + varint_sum(predictor_residuals(samples, kind).iter()),
    })
}

fn choose_model(samples: &[i16]) -> Model {
    let mut best_len = payload_len(MODEL_RAW, 0, samples).unwrap_or(usize::MAX);
    let mut best = Model {
        kind: MODEL_RAW,
        lag: 0,
    };
    for kind in [MODEL_D1, MODEL_D2] {
        if let Some(n) = payload_len(kind, 0, samples) {
            if n < best_len {
                best = Model { kind, lag: 0 };
                best_len = n;
            }
        }
    }
    // dense lag sweep covers any periodicity up to 96 samples (~735 Hz @ 44.1 kHz)
    for lag in 2..=96usize {
        if let Some(n) = payload_len(MODEL_PERIOD, lag, samples) {
            if n < best_len {
                best = Model {
                    kind: MODEL_PERIOD,
                    lag,
                };
                best_len = n;
            }
        }
    }
    best
}

fn serialize(model: Model, sample_rate: u32, samples: &[i16]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(samples.len() * 2 + 64);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(model.kind);
    out.extend_from_slice(&sample_rate.to_le_bytes());
    write_varint(&mut out, samples.len() as u64);
    if model.kind == MODEL_PERIOD {
        let lag = model.lag;
        write_varint(&mut out, lag as u64);
        let (residuals, masks) = period_plan(samples, lag).ok_or(ERR_TRUNCATED)?;
        for &m in &masks {
            out.extend_from_slice(&m.to_le_bytes());
        }
        for &r in &residuals[..lag] {
            write_varint(&mut out, zigzag_encode(r));
        }
        for i in lag..samples.len() {
            if masks[(i - lag) / 32] & (1 << ((i - lag) % 32)) != 0 {
                write_varint(&mut out, zigzag_encode(residuals[i]));
            }
        }
    } else {
        for r in predictor_residuals(samples, model.kind) {
            write_varint(&mut out, zigzag_encode(r));
        }
    }
    Ok(out)
}

fn decode(payload: &[u8]) -> Result<(u32, Vec<i16>, Model), String> {
    if payload.len() < 14 || payload[..8] != *MAGIC {
        return Err(ERR_BAD_MAGIC.into());
    }
    if payload[8] != VERSION {
        return Err(format!("unsupported version {}", payload[8]));
    }
    let kind = payload[9];
    if kind > MODEL_PERIOD {
        return Err(format!("unknown model kind {kind}"));
    }
    let sample_rate = u32::from_le_bytes(payload[10..14].try_into().unwrap());
    if sample_rate == 0 {
        return Err("invalid sample rate".into());
    }
    let mut pos = 14usize;
    let count = read_varint(payload, &mut pos)? as usize;
    if count == 0 {
        return Err(ERR_EMPTY.into());
    }
    let mut lag = 0usize;
    if kind == MODEL_PERIOD {
        lag = read_varint(payload, &mut pos)? as usize;
        if lag == 0 || lag >= count {
            return Err("invalid PERIOD lag".into());
        }
    }
    let mut samples: Vec<i16> = Vec::with_capacity(count);
    match kind {
        MODEL_PERIOD => {
            let tail = count - lag;
            let mask_len = tail.div_ceil(32);
            let mut masks = Vec::with_capacity(mask_len);
            for _ in 0..mask_len {
                let b = payload.get(pos..pos + 4).ok_or(ERR_TRUNCATED)?;
                masks.push(u32::from_le_bytes(b.try_into().unwrap()));
                pos += 4;
            }
            let mut prev: Vec<i64> = Vec::with_capacity(count);
            for _ in 0..lag {
                let z = read_varint(payload, &mut pos)?;
                let v = i16_from_i64(zigzag_decode(z))?;
                prev.push(v as i64);
                samples.push(v);
            }
            for i in lag..count {
                let slot = (i - lag) / 32;
                let bit = (i - lag) % 32;
                let used_lag = masks[slot] & (1 << bit) != 0;
                // mask clear = zero residual; encoder wrote no varint for it
                let value = if used_lag {
                    let z = read_varint(payload, &mut pos)?;
                    zigzag_decode(z) + prev[i - lag]
                } else {
                    prev[i - lag]
                };
                let v = i16_from_i64(value)?;
                samples.push(v);
                prev.push(value);
            }
        }
        MODEL_D1 => {
            let mut last: Option<i64> = None;
            for _ in 0..count {
                let z = read_varint(payload, &mut pos)?;
                let r = zigzag_decode(z);
                let v = match last {
                    Some(p) => p + r,
                    None => r,
                };
                samples.push(i16_from_i64(v)?);
                last = Some(v);
            }
        }
        MODEL_D2 => {
            let mut s1: Option<i64> = None;
            let mut s2: Option<i64> = None;
            for _ in 0..count {
                let z = read_varint(payload, &mut pos)?;
                let r = zigzag_decode(z);
                let v = match (s1, s2) {
                    (None, _) => r,
                    (Some(p1), None) => p1 + r,
                    (Some(p1), Some(p2)) => p1 + (p1 - p2) + r,
                };
                samples.push(i16_from_i64(v)?);
                s2 = s1;
                s1 = Some(v);
            }
        }
        _ => {
            for _ in 0..count {
                let z = read_varint(payload, &mut pos)?;
                samples.push(i16_from_i64(zigzag_decode(z))?);
            }
        }
    }
    if pos != payload.len() {
        return Err("trailing bytes after payload".into());
    }
    Ok((
        sample_rate,
        samples,
        Model {
            kind,
            lag: if kind == MODEL_PERIOD { lag } else { 0 },
        },
    ))
}

/// Encode: model selection + serialization. Public entry point.
pub fn encode(sample_rate: u32, samples: &[i16]) -> Result<Vec<u8>, String> {
    if samples.is_empty() {
        return Err(ERR_EMPTY.into());
    }
    if sample_rate == 0 {
        return Err("invalid sample rate".into());
    }
    let model = choose_model(samples);
    serialize(model, sample_rate, samples)
}

/// Decode: exact reconstruction; also returns the model the payload used.
pub fn decode_payload(payload: &[u8]) -> Result<(u32, Vec<i16>, Model), String> {
    decode(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(rate: u32, samples: &[i16]) -> Model {
        let payload = encode(rate, samples).expect("encode");
        let (r2, s2, m2) = decode_payload(&payload).expect("decode");
        assert_eq!(r2, rate);
        assert_eq!(s2, samples, "samples must reconstruct exactly");
        m2
    }

    #[test]
    fn silence_roundtrips_and_shrinks() {
        roundtrip(44100, &[0i16; 4096]);
        let payload = encode(44100, &[0i16; 4096]).expect("encode");
        // PERIOD lag 2: mask words (128*4 B) dominate; well under 100 despite that
        assert!(
            payload.len() < 600,
            "silence should compress hard, got {} bytes",
            payload.len()
        );
    }

    #[test]
    fn sine_uses_period_model() {
        let samples: Vec<i16> = (0..5000)
            .map(|i| (10000.0 * (2.0 * std::f64::consts::PI * i as f64 / 24.0).sin()) as i16)
            .collect();
        let model = roundtrip(44100, &samples);
        assert_eq!(
            model.kind, MODEL_PERIOD,
            "fixed-frequency sine should win with PERIOD"
        );
        assert!(model.lag >= 2 && model.lag < samples.len());
    }

    #[test]
    fn extremes_and_pseudo_random_noise_roundtrip() {
        let mut samples = vec![i16::MIN, i16::MAX];
        let mut x: u64 = 0x243F6A8885A308D3;
        for _ in 0..4094 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            samples.push((x & 0xFFFF) as u16 as i16);
        }
        let model = roundtrip(48000, &samples);
        assert_eq!(model.kind, MODEL_RAW, "noise should favor RAW");
    }

    #[test]
    fn tiny_inputs_roundtrip() {
        roundtrip(44100, &[77]);
        roundtrip(44100, &[-3, 9]);
        roundtrip(44100, &[i16::MIN]);
        roundtrip(44100, &[i16::MAX, i16::MIN]);
        roundtrip(44100, &[0]);
    }

    #[test]
    fn malformed_payloads_rejected() {
        assert!(decode_payload(b"").is_err());
        assert!(decode_payload(b"RIFFxxxxWAVE").is_err());
        assert!(decode_payload(b"ACUSTCS1\xff\x01\x00").is_err()); // short header
        assert!(decode_payload(&[0u8; 15]).is_err());
        let p = encode(44100, &[1, 2, 3]).expect("encode");
        assert!(decode_payload(&p[..p.len() - 1]).is_err(), "truncation");
        assert!(
            decode_payload(&[&p[..], b"x"].concat()).is_err(),
            "trailing byte"
        );
        let mut bad = p.clone();
        bad[0] ^= 0xFF;
        assert!(decode_payload(&bad).is_err(), "bad magic");
        let mut badver = p.clone();
        badver[8] = 9;
        assert!(decode_payload(&badver).is_err(), "bad version");
        let mut badkind = p.clone();
        badkind[9] = 99;
        assert!(decode_payload(&badkind).is_err(), "bad kind");
        // count varint claims more samples than the body carries
        let mut badcount = p.clone();
        badcount[14] |= 0x40;
        assert!(decode_payload(&badcount).is_err(), "inflated count");
        // zero rate
        let mut badrate = encode(44100, &[0i16; 300]).expect("encode");
        badrate[10..14].copy_from_slice(&0u32.to_le_bytes());
        assert!(decode_payload(&badrate).is_err(), "zero rate");
    }

    #[test]
    fn wav_roundtrip_to_payload() {
        let samples: Vec<i16> = (0..1000)
            .map(|i: i32| (i * 37 % 60000 - 30000) as i16)
            .collect();
        let wav = write_wav(44100, &samples).expect("wav");
        let (rate, parsed) = parse_wav(&wav).expect("parse");
        assert_eq!(rate, 44100);
        assert_eq!(parsed, samples);
        let payload = encode(rate, &parsed).expect("encode");
        let (_, back, _) = decode_payload(&payload).expect("decode");
        assert_eq!(back, samples);
    }

    #[test]
    fn wav_rejects_8bit_and_stereo() {
        // 8-bit mono: fmt says bits=8
        let mut wav8 = Vec::new();
        wav8.extend_from_slice(b"RIFF");
        wav8.extend_from_slice(&36u32.to_le_bytes());
        wav8.extend_from_slice(b"WAVEfmt ");
        wav8.extend_from_slice(&16u32.to_le_bytes());
        wav8.extend_from_slice(&1u16.to_le_bytes());
        wav8.extend_from_slice(&1u16.to_le_bytes());
        wav8.extend_from_slice(&44100u32.to_le_bytes());
        wav8.extend_from_slice(&44100u32.to_le_bytes());
        wav8.extend_from_slice(&1u16.to_le_bytes());
        wav8.extend_from_slice(&8u16.to_le_bytes());
        wav8.extend_from_slice(b"data");
        wav8.extend_from_slice(&2u32.to_le_bytes());
        wav8.extend_from_slice(&[0u8, 255]);
        assert!(parse_wav(&wav8).is_err());

        // stereo 16-bit: channels=2, but fmt size 16 keeps header aligned at 36
        let mut wav2 = Vec::new();
        wav2.extend_from_slice(b"RIFF");
        wav2.extend_from_slice(&36u32.to_le_bytes());
        wav2.extend_from_slice(b"WAVEfmt ");
        wav2.extend_from_slice(&16u32.to_le_bytes());
        wav2.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav2.extend_from_slice(&2u16.to_le_bytes()); // stereo
        wav2.extend_from_slice(&44100u32.to_le_bytes());
        wav2.extend_from_slice(&44100u32.to_le_bytes());
        wav2.extend_from_slice(&4u16.to_le_bytes());
        wav2.extend_from_slice(&16u16.to_le_bytes());
        wav2.extend_from_slice(b"data");
        wav2.extend_from_slice(&4u32.to_le_bytes());
        wav2.extend_from_slice(&[0u8, 0, 0, 0]);
        assert!(parse_wav(&wav2).is_err());
    }

    #[test]
    fn encode_rejects_empty_and_zero_rate() {
        assert!(encode(44100, &[]).is_err());
        assert!(encode(0, &[0i16; 4]).is_err());
    }

    #[test]
    fn zigzag_varint_roundtrip() {
        for v in [
            0i64,
            1,
            -1,
            63,
            -64,
            64,
            -65,
            8191,
            -8192,
            i16::MAX as i64,
            i16::MIN as i64,
            30000,
            -30000,
            1 << 40,
            -(1 << 40),
        ] {
            let z = zigzag_encode(v);
            let mut buf = Vec::new();
            write_varint(&mut buf, z);
            let mut pos = 0;
            let back = read_varint(&buf, &mut pos).expect("varint");
            assert_eq!(zigzag_decode(back), v, "value {v}");
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn payload_len_matches_serialize_for_every_model() {
        // selection metric must equal serialize's actual byte count for EVERY candidate
        // (would have caught any header/body drift between the two implementations)
        let samples: Vec<i16> = (0..600i64)
            .map(|i| {
                if i % 97 < 90 {
                    (2000 * (i % 13)) as i16 // quasi-periodic run
                } else {
                    let x = (i * 811) % 65536; // deterministic noise bursts
                    (x as i32 - 32768) as i16
                }
            })
            .collect();
        for &kind in &[MODEL_RAW, MODEL_D1, MODEL_D2] {
            let n = payload_len(kind, 0, &samples).expect("len");
            let ser = serialize(Model { kind, lag: 0 }, 44100, &samples).expect("ser");
            assert_eq!(n, ser.len(), "kind {kind}: metric must equal serialize");
            let (_, back, _) = decode_payload(&ser).expect("dec");
            assert_eq!(back, samples);
        }
        for lag in 2..=96usize {
            if lag >= samples.len() {
                continue;
            }
            let n = payload_len(MODEL_PERIOD, lag, &samples).expect("len");
            let ser = serialize(
                Model {
                    kind: MODEL_PERIOD,
                    lag,
                },
                44100,
                &samples,
            )
            .expect("ser");
            assert_eq!(n, ser.len(), "lag {lag}: metric must equal serialize");
            let (_, back, _) = decode_payload(&ser).expect("dec");
            assert_eq!(back, samples);
        }
    }
}
