// Audio-G1 v0.7 battery measurement: 8 fixture classes x 4096 samples.
// Per-arm OP costs, then portfolio-smallest vs the standing models.
use acoustics::{encode, research};
fn main() {
    let n = 4096usize;
    let mut xr: u64 = 0x243F6A8885A308D3;
    let mut rnd = move || {
        xr ^= xr << 13;
        xr ^= xr >> 7;
        xr ^= xr << 17;
        (xr & 0xFFFF) as u16 as i16
    };

    // 1. integer ramp (exact D2 class)
    let ramp: Vec<i16> = (0..n).map(|i| (i * 3 % 65536) as i16).collect();
    // 2. square lattice: exact integer phase square wave, period 64, amplitude 10000
    let square: Vec<i16> = (0..n)
        .map(|i| if (i / 32) % 2 == 0 { 10000 } else { -10000 })
        .collect();
    // 3. triangle lattice: period 128 integer triangle
    let tri: Vec<i16> = (0..n)
        .map(|i| {
            let p = i % 128;
            let v = if p < 64 {
                -10000 + (p as i32) * 320
            } else {
                10000 - ((p - 64) as i32) * 320
            };
            v as i16
        })
        .collect();
    // 4. floating damped tone (the independent-float class PERIOD won in G1)
    let tone: Vec<i16> = (0..n)
        .map(|i| {
            let t = i as f64 / 44100.0;
            (12000.0 * (-18400.0 * t).exp() * (2.0 * std::f64::consts::PI * 917.3 * t + 0.37).cos())
                .clamp(-32767.0, 32767.0) as i16
        })
        .collect();
    // 5. mixed harmonics: 3 partials, small noise
    let mixed: Vec<i16> = (0..n)
        .map(|i| {
            let t = i as f64 / 44100.0;
            let v = 8000.0 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                + 5000.0 * (2.0 * std::f64::consts::PI * 880.0 * t).sin()
                + 3000.0 * (2.0 * std::f64::consts::PI * 1320.0 * t).sin()
                + 200.0 * ((i * 2654435761 % 4294967296) as f64 / 4294967296.0 - 0.5);
            v.clamp(-32767.0, 32767.0) as i16
        })
        .collect();
    // 6. piecewise: silence / ramp / square / noise segments
    let mut piece: Vec<i16> = vec![0; n];
    for (i, v) in piece.iter_mut().enumerate() {
        *v = match i / 1024 {
            0 => 0,
            1 => ((i % 1024) * 30 % 65536) as i16,
            2 => {
                if (i / 16) % 2 == 0 {
                    8000
                } else {
                    -8000
                }
            }
            _ => rnd(),
        };
    }
    // 7. speech-like: sum of formant-ish partials + envelope + noise
    let speech: Vec<i16> = (0..n)
        .map(|i| {
            let t = i as f64 / 44100.0;
            let env = (6.0 * t).sin().abs().max(0.1);
            let v = env
                * (3000.0 * (2.0 * std::f64::consts::PI * 120.0 * t).sin()
                    + 2000.0 * (2.0 * std::f64::consts::PI * 700.0 * t).sin()
                    + 1500.0 * (2.0 * std::f64::consts::PI * 1220.0 * t).sin());
            (v + rnd() as f64 / 8.0).clamp(-32767.0, 32767.0) as i16
        })
        .collect();
    let noise: Vec<i16> = (0..n).map(|_| rnd()).collect();

    let fixtures: [(&str, &Vec<i16>); 8] = [
        ("ramp", &ramp),
        ("square", &square),
        ("triangle", &tri),
        ("damped", &tone),
        ("mixed", &mixed),
        ("piecewise", &piece),
        ("speech", &speech),
        ("noise", &noise),
    ];

    println!("fixture pcm_bytes portfolio portfolio_pct per_arm_wins arm_costs");
    for (name, samples) in fixtures {
        let pcm = samples.len() * 2;
        let payload = encode(44100, samples).expect("encode");
        let arms: Vec<String> = (0..acoustics::OP_ARM_COUNT)
            .filter_map(|op| research::op_arm_len(samples, op).map(|c| format!("{op}:{c}")))
            .collect();
        let best_op = (0..acoustics::OP_ARM_COUNT)
            .filter_map(|op| research::op_arm_len(samples, op))
            .min();
        let pct = payload.len() as f64 / pcm as f64 * 100.0;
        println!(
            "{name} {} {} {:.2} {} [{}]",
            pcm,
            payload.len(),
            pct,
            if best_op == Some(payload.len()) {
                "OP"
            } else {
                "-"
            },
            arms.join(" ")
        );
    }
}
