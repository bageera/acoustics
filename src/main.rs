use std::env;
use std::fs;
use std::process;

const USAGE: &str = "ACOUSTICS — Mathematical & Physics-Informed Audio Research Engine v0.1

USAGE:
    acoustics benchmark <input.wav> <output.acoust>   encode + print model/size stats
    acoustics decode <input.acoust> <output.wav>      decode exactly, write 16-bit WAV
    acoustics info <input.acoust>                     describe a payload

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

fn decode_cmd(in_path: &str, out_path: &str) -> Result<(), String> {
    let payload = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
    let (rate, samples, model) = acoustics::decode_payload(&payload)?;
    let wav = acoustics::write_wav(rate, &samples)?;
    fs::write(out_path, &wav).map_err(|e| format!("write {out_path}: {e}"))?;
    println!(
        "decoded {} samples @ {rate} Hz ({}) -> {out_path} (exact)",
        samples.len(),
        model_name(&model)
    );
    Ok(())
}

fn info_cmd(in_path: &str) -> Result<(), String> {
    let payload = fs::read(in_path).map_err(|e| format!("read {in_path}: {e}"))?;
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
