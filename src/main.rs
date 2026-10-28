use std::env;
use std::fs;
use std::process;

const USAGE: &str = "ACOUSTICS — Mathematical & Physics-Informed Audio Research Engine v0.1

USAGE:
    acoustics benchmark <input.wav> <output.acoust>   encode + print model/size stats
    acoustics decode <input.acoust> <output.wav>      decode exactly, write 16-bit WAV
    acoustics info <input.acoust>                     describe a payload

Supports mono, 16-bit PCM WAV input.";

fn main() {
    let args: Vec<String> = env::args().collect();
    let (cmd, operands) = match args.len() {
        3 => (args[1].as_str(), &args[2..3]),
        4 => (args[1].as_str(), &args[2..4]),
        _ => (
            args.get(1).map(String::as_str).unwrap_or(""),
            &[] as &[String],
        ),
    };
    let result = match (cmd, operands.len()) {
        ("benchmark", 2) => benchmark(&operands[0], &operands[1]),
        ("decode", 2) => decode_cmd(&operands[0], &operands[1]),
        ("info", 1) => info_cmd(&operands[0]),
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

fn model_name(model: &acoustics::Model) -> String {
    match model.kind {
        acoustics::MODEL_RAW => "RAW".into(),
        acoustics::MODEL_D1 => "D1 (first-order differences)".into(),
        acoustics::MODEL_D2 => "D2 (second-order differences)".into(),
        acoustics::MODEL_PERIOD => format!("PERIOD (lag {})", model.lag),
        _ => format!("unknown({})", model.kind),
    }
}
