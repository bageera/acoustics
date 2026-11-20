//! N26-B — the auxiliary renderer's performance qualification
//! (`--release --ignored --nocapture`). Workloads, real-time factors,
//! block latencies, per-rate throughput, allocation counts (init vs
//! steady state), seek latency, and streamed long-duration RSS.
//!
//! Measured separately from the codec's encode memory: the unresolved
//! 4 GB G23 target is NOT addressed here (this subsystem never touches the
//! compression engine).

#![cfg(test)]

use super::descriptor::StreamDescriptor;
use super::wire::AuxSpec;
use super::{mix_into_primary, parse, render_window};
use std::time::Instant;

fn theta_am(mhz: u32, amp: u16, depth: u16, coupling: u8) -> StreamDescriptor {
    StreamDescriptor {
        model: 1,
        flags: 1,
        carrier_mhz: mhz,
        carrier_phase_q16: 0,
        amplitude_q15: amp,
        mod_mhz: 5859,
        mod_depth_q16: depth,
        mod_phase_q16: 0,
        start_sample: 0,
        duration_samples: 0,
        routing: 3,
        coupling_ref: coupling,
    }
}

fn sidecar(workload: &str) -> AuxSpec {
    let payload = vec![0x11u8; 256];
    let sha = crate::golden::sha256_for_aux(&payload);
    let mut binding = [0u8; 8];
    binding.copy_from_slice(&sha[..8]);
    let streams = match workload {
        "single_tone" => vec![StreamDescriptor {
            model: 0,
            flags: 1,
            carrier_mhz: 40000,
            carrier_phase_q16: 0,
            amplitude_q15: 8192,
            mod_mhz: 0,
            mod_depth_q16: 0,
            mod_phase_q16: 0,
            start_sample: 0,
            duration_samples: 0,
            routing: 3,
            coupling_ref: 0,
        }],
        "uncoupled_dual" => vec![
            theta_am(40000, 8192, 32768, 0),
            StreamDescriptor {
                model: 1,
                flags: 1,
                carrier_mhz: 40400,
                carrier_phase_q16: 21845,
                amplitude_q15: 4096,
                mod_mhz: 6300,
                mod_depth_q16: 24576,
                mod_phase_q16: 16384,
                start_sample: 0,
                duration_samples: 0,
                routing: 1,
                coupling_ref: 0,
            },
        ],
        "coupled_dual" => vec![theta_am(40000, 8192, 32768, 0), {
            let mut s = theta_am(40400, 4096, 32768, 1);
            s.routing = 2;
            s
        }],
        _ => unreachable!(),
    };
    AuxSpec { streams, binding }
}

/// render n samples through 1024-sample windows (the streaming shape).
fn streamed(spec: &AuxSpec, rate: u32, n: usize, block: usize) -> (Instant, u64) {
    let t0 = Instant::now();
    let mut clamped = 0u64;
    let mut start = 0usize;
    let buf0 = vec![0i16; block];
    let buf1 = vec![0i16; block];
    let mut chans = vec![buf0, buf1];
    while start < n {
        let len = block.min(n - start);
        clamped += render_window(spec, rate, n as u64, start as u64, len, &mut chans)
            .unwrap()
            .clamped;
        start += len;
    }
    (t0, clamped)
}

#[test]
#[ignore]
fn n26_performance_qualification() {
    let rate = 48000u32;
    let seconds = 10usize;
    let n = rate as usize * seconds;
    println!("== N26-B rendering throughput (release; RTF = render/audio) ==");

    for workload in ["single_tone", "uncoupled_dual", "coupled_dual"] {
        let spec = sidecar(workload);
        let (t0, _) = streamed(&spec, rate, n, 1024);
        let elapsed = t0.elapsed();
        let rtf = elapsed.as_secs_f64() / seconds as f64;
        println!(
            "  {workload:14}: {} samples in {:>8.1?} → RTF {rtf:.4} ({:.1} Msamples/s)",
            n,
            elapsed,
            n as f64 / elapsed.as_secs_f64() / 1e6
        );
        assert!(rtf < 1.0, "{workload} must render faster than real time");
    }

    println!("  -- block latency (coupled_dual, 48 kHz) --");
    let spec = sidecar("coupled_dual");
    for block in [64usize, 128, 256, 512, 1024] {
        let (t0, _) = streamed(&spec, rate, n, block);
        let per = t0.elapsed().as_secs_f64() * 1e9 / (n / block) as f64;
        println!("    block {block:>5}: {per:.0} ns/render-window avg");
    }

    println!("  -- per-rate throughput (coupled_dual, 10 s) --");
    for r in [8000u32, 44100, 48000, 96000, 192000] {
        let n = r as usize * seconds;
        let (t0, _) = streamed(&spec, r, n, 1024);
        let s = t0.elapsed().as_secs_f64();
        println!(
            "    {r:>6} Hz: {} samples in {s:.2} s (RTF {:.4})",
            n,
            s / seconds as f64
        );
    }

    println!("  -- allocations (init vs steady state) --");
    crate::alloc_meter::reset();
    let _ = parse(&sidecar("single_tone").write()); // one write + parse
    let init = crate::alloc_meter::get();
    crate::alloc_meter::reset();
    let (t0, _) = streamed(&spec, rate, n, 1024);
    let steady_state = crate::alloc_meter::get();
    println!(
        "    init {init} allocs; steady-state over {n} samples in 1024-windows: {steady_state} (render time {:?})",
        t0.elapsed()
    );
    assert!(
        steady_state < n / 1024 + 64,
        "steady-state allocations must stay O(windows), not O(samples)"
    );

    println!("  -- seek latency (arbitrary offsets incl. boundaries) --");
    let buf = vec![0i16; 1024];
    let chans = vec![buf.clone(), buf];
    for &offset in &[0u64, 1, 10_000, n as u64 - 1024, n as u64] {
        let t0 = Instant::now();
        let r = render_window(&spec, rate, n as u64, offset, 1024, &mut chans.clone());
        let ok = r.is_ok();
        println!(
            "    offset {offset:>12}: {:?} ({} at the boundary)",
            t0.elapsed(),
            if offset >= n as u64 {
                "silent-window"
            } else if ok {
                "ok"
            } else {
                "rejected"
            }
        );
    }

    println!("  -- streamed long-duration RSS (60 s @ 48 kHz, 1024-blocks) --");
    let n60 = rate as usize * 60;
    let (t0, _) = streamed(&spec, rate, n60, 1024);
    let pid = std::process::id();
    let rss = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<usize>()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    println!(
        "    2.88M samples in {:?}; test-binary RSS ~{} KB (constant-window rendering)",
        t0.elapsed(),
        rss
    );

    println!("  -- mixed playback (mono + stereo) with the headroom policy --");
    let primary_l: Vec<i16> = (0..48_000)
        .map(|i| ((i * 7 % 31_000) as i64) as i16)
        .collect();
    let mut primary_stereo = vec![primary_l.clone(), primary_l.clone()];
    let t0 = Instant::now();
    let rep_m = mix_into_primary(
        &sidecar("uncoupled_dual"),
        rate,
        48_000,
        &mut [primary_l.clone()],
    );
    let rep_s = mix_into_primary(
        &sidecar("uncoupled_dual"),
        rate,
        48_000,
        &mut primary_stereo,
    );
    println!(
        "    mono mix: scale_q16 {}, clamped {}, {:?}",
        rep_m.as_ref().map(|r| r.scale_q16).unwrap_or(0),
        rep_m.as_ref().map(|r| r.clamped).unwrap_or(0),
        t0.elapsed()
    );
    println!(
        "    stereo mix: scale_q16 {}, clamped {}",
        rep_s.as_ref().map(|r| r.scale_q16).unwrap_or(0),
        rep_s.as_ref().map(|r| r.clamped).unwrap_or(0)
    );
    println!("\nN26-B measured. The 4 GB codec-encode target is untouched by this phase.");
}
