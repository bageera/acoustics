//! v5-S — state-conditioned residual coding (research-only; the v4 wire and
//! the released 0.8.2 behavior stay frozen).
//!
//! Hypothesis (the G5-F review): the residual's PROBABILITY should depend on
//! the decoder's own recurrence state — the residual VALUES stay unchanged;
//! only the coding probabilities vary:
//!
//!   r[n] = x[n] - xhat[n]  (frozen reconstruction; residual exact)
//!   bucket = f(|y[n-1]|)   (fixed thresholds, no learned wire metadata)
//!
//! Decoder causality: the bucket's state y[n-1] is RECONSTRUCTED before r[n]
//! is decoded — the G5 constraint, honored structurally (the state comes from
//! the replay's own reconstructed stream).
//!
//! Measurements per S-row (the review's three quantities):
//! 1. dH = H0 - H(buckets) (context information)
//! 2. dL_coder = ideal bits, global vs bucketed histograms
//! 3. dL_complete = the above + HONEST table transport (each histogram entry
//!    costs ~4 B) + framing
//!
//! S1 = one global histogram (control); S2 = 2 buckets; S3 = 4 buckets;
//! S5 = 4 magnitude buckets (same machinery, thresholds on |y|); S6 = the
//! randomized-assignment negative control (must NOT win).
//! Held-out rule: thresholds fixed from the synthetic two-tone fixture; the
//! locked real recordings never tune them.

#[allow(dead_code)] // instruments live for the next v5-S round
fn bucket_of(state_mag: i64, thresholds: &[i64]) -> usize {
    let mut b = 0usize;
    for (i, &t) in thresholds.iter().enumerate() {
        if state_mag >= t {
            b = i + 1;
        }
    }
    b
}

/// Ideal bits under per-bucket histograms + the table transport byte estimate
/// (every (value, count) histogram row billed ~4 B — the honest both-sides
/// constant-table cost of a static context coder).
#[allow(dead_code)] // instruments live for the next v5-S round
fn ideal_bits(per_pos: &[i64], ctx_mags: &[i64], thresholds: &[i64]) -> (f64, usize) {
    let n = per_pos.len().min(ctx_mags.len());
    let nb = thresholds.len() + 1;
    let mut hists: Vec<std::collections::HashMap<i64, u32>> =
        (0..nb).map(|_| std::collections::HashMap::new()).collect();
    let mut sizes = vec![0usize; nb];
    for i in 0..n {
        let b = bucket_of(ctx_mags[i], thresholds).min(nb - 1);
        *hists[b].entry(per_pos[i]).or_insert(0) += 1;
        sizes[b] += 1;
    }
    let mut total = 0f64;
    for (b, h) in hists.iter().enumerate() {
        if sizes[b] == 0 {
            continue;
        }
        let cnt = sizes[b] as f64;
        total += -h
            .values()
            .map(|&c| {
                let p = c as f64 / cnt;
                p * p.log2()
            })
            .sum::<f64>()
            * cnt;
    }
    let entries: usize = hists.iter().map(|h| h.len()).sum();
    (total, entries * 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g5::anatomy_positions as positions;

    const TH_SYNTH_2: [i64; 1] = [12288]; // 3/8 full scale (synthetic-frozen)
    const TH_SYNTH_4: [i64; 3] = [8192, 16384, 24576];

    fn fixture_two_tone() -> Vec<i16> {
        (0..4096usize)
            .map(|i| {
                let t = i as f64 / 44100.0;
                let v = 8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
                (v.round()).clamp(-32767.0, 32767.0) as i16
            })
            .collect()
    }

    /// S1/S2/S3/S5 on the synthetic fixture (thresholds frozen here) + the
    /// randomized negative control (S6, same machinery, randomized contexts).
    #[test]
    fn v5s_state_conditioned_sweep() {
        let x = fixture_two_tone();
        let Some((r, mags)) = positions(&x, 1) else {
            panic!("stage0 positions required");
        };
        let bits_bytes = |h: f64, t: usize| (h / 8.0).ceil() as usize + t;
        let (h_global, t_global) = ideal_bits(&r, &mags, &[]);
        let (h_2, t_2) = ideal_bits(&r, &mags, &TH_SYNTH_2);
        let (h_4, t_4) = ideal_bits(&r, &mags, &TH_SYNTH_4);
        eprintln!(
            "v5-S synthetic n={}: S1={}B S2={}B S3/5={}B",
            r.len(),
            bits_bytes(h_global, t_global),
            bits_bytes(h_2, t_2),
            bits_bytes(h_4, t_4)
        );
        // S6: randomized contexts (negative control)
        let mut xr: u64 = 0x9E37_79B9_7F4A_7C15;
        let shuffled: Vec<i64> = (0..mags.len())
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (xr % 30000) as i64
            })
            .collect();
        let (h_rand, t_rand) = ideal_bits(&r, &shuffled, &TH_SYNTH_4);
        eprintln!(
            "v5-S S6 randomized: {}B (must NOT beat S3/5's {}B)",
            bits_bytes(h_rand, t_rand),
            bits_bytes(h_4, t_4)
        );
        assert!(
            bits_bytes(h_rand, t_rand) >= bits_bytes(h_4, t_4),
            "S6: randomized context must not beat the state-conditioned model"
        );
    }

    /// The held-out rule: the synthetic-frozen thresholds measured unchanged on
    /// the three locked real recordings (testdata/g5f) — the promotion-gate
    /// numbers, compared against the SHIPPED coder's cost on the same windows.
    #[test]
    fn v5s_held_out_real_corpus() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let files = ["example_word.wav", "can_you_stop.wav", "sample2.wav"];
        for name in files {
            let data = std::fs::read(format!("{dir}/{name}")).expect("run g5f_fetch.sh");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, stream) in streams.iter().enumerate() {
                let wins = stream.len() / 4096;
                let (mut h1, mut h2, mut h4, mut t1, mut t2, mut t4) = (0f64, 0f64, 0f64, 0, 0, 0);
                let mut shipped = 0usize;
                for w in 0..wins {
                    let seg = &stream[w * 4096..(w + 1) * 4096];
                    shipped += crate::encode(rate, seg).expect("e").len();
                    if let Some((r, mags)) = positions(seg, 1) {
                        let (h, t) = ideal_bits(&r, &mags, &[]);
                        h1 += h;
                        t1 += t;
                        let (h, t) = ideal_bits(&r, &mags, &TH_SYNTH_2);
                        h2 += h;
                        t2 += t;
                        let (h, t) = ideal_bits(&r, &mags, &TH_SYNTH_4);
                        h4 += h;
                        t4 += t;
                    }
                }
                let bb = |h: f64, t: usize| (h / 8.0).ceil() as usize + t;
                eprintln!(
                    "v5-S {name} ch{ch}: shipped={shipped}B S1={}B S2={}B S3/5={}B across {wins}w",
                    bb(h1, t1),
                    bb(h2, t2),
                    bb(h4, t4)
                );
            }
        }
    }
}
