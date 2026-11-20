use std::env;
use std::fs;
use std::process;

const USAGE: &str = "ACOUSTICS — Mathematical & Physics-Informed Audio Research Engine v0.1

USAGE:
    acoustics benchmark <input.wav> <output.acoust>   mono encode + stats (the G23-G path)
    acoustics encode <input.wav> <output.acoust>      encode MONO OR STEREO (v8, Balanced);
                                                      verifies per-channel PCM identity in-process
    acoustics decode <input.acoust> <output.wav>      decode exactly, write 16-bit WAV
                                                      (v8 stereo/mono + historical payloads)
    acoustics info <input.acoust>                     describe a payload (v8 or historical)

Auxiliary mathematical streams (the .acaux research sidecar; NEVER
mixed into archival decoding — decode reconstructs original PCM only):
    acoustics aux inspect <file.acaux>                list descriptors
    acoustics aux verify <payload> <file.acaux>       binding + grammar check
    acoustics aux render <payload> <file.acaux> <out.wav>   aux signal only
    acoustics aux mix <payload> <file.acaux> <out.wav>      derived playback
      (the mix applies the headroom policy; it is a DERIVED artifact,
       not the archival representation)

Supports mono, 16-bit PCM WAV input.";

fn main() {
    let args: Vec<String> = env::args().collect();
    let (cmd, operands) = match args.len() {
        3 => (args[1].as_str(), &args[2..3]),
        4 => (args[1].as_str(), &args[2..4]),
        5 => (args[1].as_str(), &args[2..5]),
        _ => (
            args.get(1).map(String::as_str).unwrap_or(""),
            &args[2..] as &[String],
        ),
    };
    let result = match (cmd, operands.len()) {
        ("benchmark", 2) => benchmark(&operands[0], &operands[1]),
        ("encode", 2) => encode_cmd(&operands[0], &operands[1]),
        ("decode", 2) => decode_cmd(&operands[0], &operands[1]),
        ("info", 1) => info_cmd(&operands[0]),
        ("aux", 2) if operands[0] == "inspect" => aux_inspect(&operands[1]),
        ("aux", 3) if operands[0] == "verify" => aux_verify(&operands[1], &operands[2]),
        ("aux", 4) if operands[0] == "render" => {
            aux_render(&operands[1], &operands[2], &operands[3])
        }
        ("aux", 4) if operands[0] == "mix" => aux_mix(&operands[1], &operands[2], &operands[3]),
        _ => Err(format!("wrong arguments for '{cmd}'\n{USAGE}")),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn benchmark(in_path: &str, out_path: &str) -> Result<(), String> {
    let wav = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
    let (rate, samples) = acoustics::parse_wav(&wav)?;
    let payload = acoustics::encode(rate, &samples)?;
    fs::write(out_path, &payload).map_err(|e| format!("write {out_path}: {e}"))?;
    let (_, _, model) = acoustics::decode_payload(&payload)?;
    let pcm_bytes = samples.len() * 2;
    let pct = (payload.len() as f64 / pcm_bytes as f64) * 100.0;
    println!(
        "input:  {in_path}  ({} samples @ {rate} Hz, {pcm_bytes} bytes PCM)",
        samples.len()
    );
    println!(
        "model:  {} ({} bytes, {:.1}% of PCM)",
        model_name(&model),
        payload.len(),
        pct
    );
    println!("output: {out_path}");
    Ok(())
}

/// G24-E: the stereo/mono v8 encode command — the EXISTING library
/// selection functions, no new codec arithmetic; verifies per-channel PCM
/// identity in-process (decode-back, channel ordering preserved, the
/// interleaved stream checked against the de-interleaved source).
fn encode_cmd(in_path: &str, out_path: &str) -> Result<(), String> {
    let wav = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
    let (rate, chans) = acoustics::parse_wav_multichannel(&wav)?;
    let t0 = std::time::Instant::now();
    let payload = match chans.len() {
        1 => acoustics::encode_v8_select_mono(rate, &chans[0], acoustics::V8Profile::Balanced)?,
        2 => {
            acoustics::encode_v8_select(rate, &chans[0], &chans[1], acoustics::V8Profile::Balanced)?
        }
        n => return Err(format!("{n}-channel input; 1 or 2 supported")),
    };
    let enc_ms = t0.elapsed().as_millis();
    fs::write(out_path, &payload).map_err(|e| format!("write {out_path}: {e}"))?;
    // in-process verification: decode-back, exact per channel AND interleaved
    let (drate, dchans) = acoustics::decode_v8(&payload)?;
    if drate != rate || dchans.len() != chans.len() {
        return Err("encode verification: dimensions differ".to_string());
    }
    let pcm_bytes = chans.len() * chans[0].len() * 2;
    let exact = (0..chans.len()).all(|c| dchans[c] == chans[c]);
    if !exact {
        return Err("encode verification: per-channel PCM mismatch".to_string());
    }
    let pct = (payload.len() as f64 / pcm_bytes as f64) * 100.0;
    println!(
        "encoded {}ch {} samples @ {rate} Hz -> {out_path} ({} bytes, {:.1}% of PCM, {enc_ms} ms) — decoded back EXACT per channel",
        chans.len(),
        chans[0].len(),
        payload.len(),
        pct
    );
    Ok(())
}

/// The historical/v8 decode dispatch (the public dispatcher keeps
/// accepting the documented historical versions).
fn wire_decode(payload: &[u8]) -> Result<(u32, Vec<Vec<i16>>), String> {
    if payload.len() >= 9 && payload[8] == acoustics::V8_VERSION {
        acoustics::decode_v8(payload)
    } else {
        let (rate, samples, _) = acoustics::decode_payload(payload)?;
        Ok((rate, vec![samples]))
    }
}

fn decode_cmd(in_path: &str, out_path: &str) -> Result<(), String> {
    let payload = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
    let (rate, chans) = wire_decode(&payload)?;
    let wav = match chans.len() {
        1 => acoustics::write_wav(rate, &chans[0])?,
        2 => acoustics::write_wav_stereo(rate, &chans[0], &chans[1])?,
        n => return Err(format!("{n}-channel payload; 1 or 2 supported")),
    };
    fs::write(out_path, &wav).map_err(|e| format!("write {out_path}: {e}"))?;
    println!(
        "decoded {}ch {} samples @ {rate} Hz -> {out_path} (exact)",
        chans.len(),
        chans[0].len()
    );
    Ok(())
}

fn info_cmd(in_path: &str) -> Result<(), String> {
    let payload = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
    if payload.len() >= 9 && payload[8] == acoustics::V8_VERSION {
        let (rate, chans) = acoustics::decode_v8(&payload)?;
        println!(
            "{in_path}: wire v8, {} channel(s), {} samples @ {rate} Hz, {} bytes",
            chans.len(),
            chans[0].len(),
            payload.len()
        );
        return Ok(());
    }
    let (rate, samples, model) = acoustics::decode_payload(&payload)?;
    println!(
        "{in_path}: model {}, {} samples @ {rate} Hz, {} bytes",
        model_name(&model),
        samples.len(),
        payload.len()
    );
    Ok(())
}

// ------------------------------ the .acaux auxiliary-stream CLI (N25) ----

/// Decode the primary's dimensions: v8 first, then the v7 grammar.
fn primary_info(payload: &[u8]) -> Result<(u32, Vec<Vec<i16>>), String> {
    match acoustics::decode_v8(payload) {
        Ok((rate, chans)) => Ok((rate, chans)),
        Err(_) => {
            let (rate, samples, _) = acoustics::decode_payload(payload)?;
            Ok((rate, vec![samples]))
        }
    }
}

fn aux_inspect(sidecar_path: &str) -> Result<(), String> {
    let bytes = fs::read(sidecar_path).map_err(|e| format!("read {sidecar_path}: {e}"))?;
    let handle = acoustics::aux::parse(&bytes)?;
    let spec = handle.spec();
    println!(
        "{sidecar_path}: ACAUX v1, {} stream(s), {} bytes",
        spec.streams.len(),
        bytes.len()
    );
    for (i, s) in spec.streams.iter().enumerate() {
        let kind = if s.model == 1 {
            "theta-AM gamma"
        } else {
            "sine"
        };
        println!(
            "  stream {}: {kind}, carrier {} Hz ({:.2}), phase {} turns, amp {}, routing {:#04x}",
            i + 1,
            s.carrier_mhz as f64 / 1000.0,
            s.carrier_mhz as f64 / 1000.0,
            s.carrier_phase_q16 as f64 / 65536.0,
            s.amplitude_q15 as f64 / 32768.0,
            s.routing
        );
        if s.model == 1 {
            println!(
                "    modulation: {} Hz, depth {:.3}, coupled-to {}",
                s.mod_mhz as f64 / 1000.0,
                s.mod_depth_q16 as f64 / 65536.0,
                if s.coupling_ref == 0 {
                    "-".to_string()
                } else {
                    format!("stream {}", s.coupling_ref)
                }
            );
        }
        println!(
            "    schedule: start {}, duration {} (0 = to the primary's end)",
            s.start_sample, s.duration_samples
        );
    }
    Ok(())
}

fn aux_verify(payload_path: &str, sidecar_path: &str) -> Result<(), String> {
    let payload = fs::read(payload_path).map_err(|e| format!("read {payload_path}: {e}"))?;
    let bytes = fs::read(sidecar_path).map_err(|e| format!("read {sidecar_path}: {e}"))?;
    let handle = acoustics::aux::parse(&bytes)?;
    let info = handle.verify_binding(&payload)?;
    println!(
        "BINDING OK: {sidecar_path} matches {payload_path} ({} samples @ {} Hz, grammar valid)",
        info.sample_count, info.rate
    );
    Ok(())
}

fn aux_render(payload_path: &str, sidecar_path: &str, out_path: &str) -> Result<(), String> {
    let payload = fs::read(payload_path).map_err(|e| format!("read {payload_path}: {e}"))?;
    let bytes = fs::read(sidecar_path).map_err(|e| format!("read {sidecar_path}: {e}"))?;
    let handle = acoustics::aux::parse(&bytes)?;
    let info = handle.verify_binding(&payload)?;
    let n = info.sample_count as usize;
    let spec = handle.spec();
    let channels = 2usize; // render is stereo-capable; streams route by mask
    let mut chans = vec![vec![0i16; n]; channels];
    let audit =
        acoustics::aux::render_window(spec, info.rate, info.sample_count, 0, n, &mut chans)?;
    let (l, r) = (&chans[0], &chans[1]);
    let stereo = spec.streams.iter().any(|s| s.routing & 0b10 != 0);
    let (rate, ls, rs): (u32, &[i16], &[i16]) = (info.rate, l.as_slice(), r.as_slice());
    let w = if stereo {
        acoustics::write_wav_stereo(rate, ls, rs)?
    } else {
        acoustics::write_wav(rate, ls)?
    };
    fs::write(out_path, &w).map_err(|e| format!("write {out_path}: {e}"))?;
    println!(
        "AUXILIARY SIGNAL ONLY (a derived artifact; the archival payload was not touched): \
{} samples @ {rate} Hz, clamped {} -> {out_path}",
        n, audit.clamped
    );
    Ok(())
}

fn aux_mix(payload_path: &str, sidecar_path: &str, out_path: &str) -> Result<(), String> {
    let payload = fs::read(payload_path).map_err(|e| format!("read {payload_path}: {e}"))?;
    let bytes = fs::read(sidecar_path).map_err(|e| format!("read {sidecar_path}: {e}"))?;
    let handle = acoustics::aux::parse(&bytes)?;
    let info = handle.verify_binding(&payload)?;
    let (_, chans) = primary_info(&payload)?;
    let n = chans[0].len();
    let mut chans = chans;
    let report =
        acoustics::aux::mix_into_primary(handle.spec(), info.rate, info.sample_count, &mut chans)?;
    let (rate, w) = if chans.len() == 2 {
        (
            info.rate,
            acoustics::write_wav_stereo(info.rate, &chans[0], &chans[1])?,
        )
    } else {
        (info.rate, acoustics::write_wav(info.rate, &chans[0])?)
    };
    fs::write(out_path, &w).map_err(|e| format!("write {out_path}: {e}"))?;
    let dsha = acoustics::aux::crc16_ccitt_false(&bytes);
    println!(
        "DERIVED PLAYBACK MIX — NOT the archival PCM (acoustics v{}, descriptor crc16 {:#06x})",
        env!("CARGO_PKG_VERSION"),
        dsha
    );
    println!(
        "headroom: Σ P_max {} PCM, primary peak {}, applied scale {:.3} (Q16 {}), clamped {} (policy: scale down or OMIT, never gain-escalate)",
        report.combined_peak,
        report.primary_peak,
        report.scale_q16 as f64 / 65536.0,
        report.scale_q16,
        report.clamped
    );
    println!("written {out_path} ({} samples @ {rate} Hz)", n);
    Ok(())
}

fn model_name(model: &acoustics::Model) -> String {
    match model.kind {
        acoustics::MODEL_RAW => "RAW".into(),
        acoustics::MODEL_D1 => "D1 (first-order differences)".into(),
        acoustics::MODEL_D2 => "D2 (second-order differences)".into(),
        acoustics::MODEL_PERIOD => format!("PERIOD (lag {})", model.param),
        acoustics::MODEL_OSC => format!("OSC (damped oscillator, variant {})", model.param),
        _ => format!("unknown({})", model.kind),
    }
}
