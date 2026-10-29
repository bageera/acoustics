//! G9 — bitstream anatomy (research-only; wire untouched).
//!
//! The review's decisive experiment: WHERE does every byte of the winning
//! representation on real speech go, measured from the REAL writer — not a
//! parallel cost estimator (the G7-A lesson).
//!
//! The ledger walks the exact byte layout the PRODUCTION serializer emits
//! (v4): same header, same field order, same encodings — verified byte-count
//! equal to `serialize()`'s payload. The review's five validator checks are
//! a test invariant on every corpus file; the output is the seven-category
//! anatomy table.
//!
//! Categories (the G9 contract):
//! 1. metadata — magic + version + kind + rate + count + kind markers
//! 2. coefficients — PERIOD lag; OSC cwq/r2q; f2 selectors; f3 stage (c,r)
//! 3. seeds — family-0 y0/y1
//! 4. masks — flat u32 words (4 B) or OP mask-RLE bytes
//! 5. residuals — zigzag varints
//! 6. segmentation — (unused on this wire: single-window payloads; kept for
//!    the category contract)
//! 7. other — OP arms' inverse-model streams (loop length, echo delay, ylen,
//!    base zigzags), RAW literal samples

use crate::{choose_model, serialize, varint_len, MODEL_OP, MODEL_OSC, MODEL_PERIOD, MODEL_RAW};

/// One accounted byte run on the wire.
struct Span {
    category: &'static str,
    begin: usize,
    end: usize,
}

const RAW_SUBMODE_RESIDUAL: u8 = 0;

/// Build the ledger by walking the production wire layout of the chosen
/// model. Returns (spans, payload). Every byte is covered exactly once.
fn ledger(model: crate::Model, rate: u32, samples: &[i16]) -> (Vec<Span>, Vec<u8>) {
    let payload = serialize(model, rate, samples).expect("serialize");
    let mut spans = Vec::new();
    let mut pos = 14usize; // magic 8 + version 1 + kind 1 + rate 4
    let meta_end = pos + varint_len(samples.len() as u64);
    spans.push(Span {
        category: "metadata",
        begin: 0,
        end: meta_end,
    });
    pos = meta_end;
    // local helpers: read a varint length at `pos` (LEB128)
    let vlen = |payload: &[u8], at: usize| -> usize {
        let mut n = 0usize;
        loop {
            let b = payload[at + n];
            n += 1;
            if b & 0x80 == 0 {
                break;
            }
        }
        n
    };
    let vint = |payload: &[u8], at: usize, n: usize| -> u64 {
        let mut v = 0u64;
        let mut shift = 0;
        for &b in payload[at..at + n].iter() {
            v |= ((b & 0x7F) as u64) << shift;
            shift += 7;
        }
        v
    };
    match model.kind {
        MODEL_RAW => {
            let n = vlen(&payload, pos);
            let sub = vint(&payload, pos, n) as u8;
            spans.push(Span {
                category: "metadata",
                begin: pos,
                end: pos + n,
            });
            pos += n;
            spans.push(Span {
                category: if sub == RAW_SUBMODE_RESIDUAL {
                    "residuals"
                } else {
                    "other"
                },
                begin: pos,
                end: payload.len(),
            });
        }
        crate::MODEL_D1 | crate::MODEL_D2 | crate::MODEL_D3 => {
            spans.push(Span {
                category: "residuals",
                begin: pos,
                end: payload.len(),
            });
        }
        MODEL_PERIOD => {
            let n = vlen(&payload, pos);
            spans.push(Span {
                category: "coefficients",
                begin: pos,
                end: pos + n,
            });
            pos += n;
            let lag = model.param;
            let mask_len = (samples.len() - lag).div_ceil(32);
            let m_end = pos + mask_len * 4;
            spans.push(Span {
                category: "masks",
                begin: pos,
                end: m_end,
            });
            pos = m_end;
            spans.push(Span {
                category: "residuals",
                begin: pos,
                end: payload.len(),
            });
        }
        MODEL_OSC => {
            let n = vlen(&payload, pos);
            spans.push(Span {
                category: "metadata",
                begin: pos,
                end: pos + n,
            });
            pos += n;
            let variant = model.param;
            let family = crate::osc_variant_family(variant);
            let grid = crate::osc_variant_grid(variant);
            let mask_len = (samples.len()).div_ceil(32) * 4; // f1/f2/f3 segment
            let mask_len0 = (samples.len() - 2).div_ceil(32) * 4; // f0 segment
            match family {
                0 => {
                    spans.push(Span {
                        category: "seeds",
                        begin: pos,
                        end: pos + 4,
                    });
                    pos += 4;
                    if grid == 2 {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                        let n2 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n2,
                        });
                        pos += n2;
                    } else {
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + 8,
                        });
                        pos += 8;
                    }
                    let m_end = pos + mask_len0;
                    spans.push(Span {
                        category: "masks",
                        begin: pos,
                        end: m_end,
                    });
                    pos = m_end;
                    spans.push(Span {
                        category: "residuals",
                        begin: pos,
                        end: payload.len(),
                    });
                }
                1 => {
                    if grid == 2 {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                        let n2 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n2,
                        });
                        pos += n2;
                    } else {
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + 8,
                        });
                        pos += 8;
                    }
                    let m_end = pos + mask_len;
                    spans.push(Span {
                        category: "masks",
                        begin: pos,
                        end: m_end,
                    });
                    pos = m_end;
                    spans.push(Span {
                        category: "residuals",
                        begin: pos,
                        end: payload.len(),
                    });
                }
                2 => {
                    spans.push(Span {
                        category: "coefficients",
                        begin: pos,
                        end: pos + 8,
                    });
                    pos += 8;
                    for _ in 0..5 {
                        let n = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n,
                        });
                        pos += n;
                    }
                    let m_end = pos + mask_len;
                    spans.push(Span {
                        category: "masks",
                        begin: pos,
                        end: m_end,
                    });
                    pos = m_end;
                    spans.push(Span {
                        category: "residuals",
                        begin: pos,
                        end: payload.len(),
                    });
                }
                _ => {
                    // f3 cascade: stage count + (c,r) zigzag pairs per stage
                    let n = vlen(&payload, pos);
                    let stages = vint(&payload, pos, n) as usize;
                    spans.push(Span {
                        category: "metadata",
                        begin: pos,
                        end: pos + n,
                    });
                    pos += n;
                    for _ in 0..stages {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                        let n2 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "coefficients",
                            begin: pos,
                            end: pos + n2,
                        });
                        pos += n2;
                    }
                    // one mask segment PER STAGE
                    for _ in 0..stages {
                        let m_end = pos + mask_len;
                        spans.push(Span {
                            category: "masks",
                            begin: pos,
                            end: m_end,
                        });
                        pos = m_end;
                    }
                    spans.push(Span {
                        category: "residuals",
                        begin: pos,
                        end: payload.len(),
                    });
                }
            }
        }
        MODEL_OP => {
            // arm id
            let n = vlen(&payload, pos);
            spans.push(Span {
                category: "metadata",
                begin: pos,
                end: pos + n,
            });
            pos += n;
            let arm = model.param;
            match arm {
                0..=9 => {
                    let n = vlen(&payload, pos);
                    let loop_len = vint(&payload, pos, n) as usize;
                    spans.push(Span {
                        category: "other",
                        begin: pos,
                        end: pos + n,
                    });
                    pos += n;
                    for _ in 0..loop_len {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "other",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                    }
                }
                10..=13 => {
                    let n = vlen(&payload, pos);
                    spans.push(Span {
                        category: "other",
                        begin: pos,
                        end: pos + n,
                    });
                    pos += n; // echo delay
                    let n = vlen(&payload, pos);
                    let ylen = vint(&payload, pos, n) as usize;
                    spans.push(Span {
                        category: "other",
                        begin: pos,
                        end: pos + n,
                    });
                    pos += n;
                    for _ in 0..ylen {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "other",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                    }
                }
                _ => {
                    let n = vlen(&payload, pos);
                    let ylen = vint(&payload, pos, n) as usize;
                    spans.push(Span {
                        category: "other",
                        begin: pos,
                        end: pos + n,
                    });
                    pos += n;
                    for _ in 0..ylen {
                        let n1 = vlen(&payload, pos);
                        spans.push(Span {
                            category: "other",
                            begin: pos,
                            end: pos + n1,
                        });
                        pos += n1;
                    }
                }
            }
            // mask RLE: varint(runs); run pairs (zero-run count, data-run
            // count+words) ALTERNATE starting with the zero run.
            let n = vlen(&payload, pos);
            let runs = vint(&payload, pos, n) as usize;
            spans.push(Span {
                category: "masks",
                begin: pos,
                end: pos + n,
            });
            pos += n;
            for run in 0..runs {
                let n = vlen(&payload, pos);
                let rc = vint(&payload, pos, n) as usize;
                spans.push(Span {
                    category: "masks",
                    begin: pos,
                    end: pos + n,
                });
                pos += n;
                if run % 2 == 1 {
                    // data run: raw u32 words
                    let w_end = pos + rc * 4;
                    spans.push(Span {
                        category: "masks",
                        begin: pos,
                        end: w_end,
                    });
                    pos = w_end;
                }
            }
            spans.push(Span {
                category: "residuals",
                begin: pos,
                end: payload.len(),
            });
        }
        _ => {}
    }
    (spans, payload)
}

/// Extract the payload's mask WORD segments (the production layout: PERIOD
/// masks after the lag varint; OSC f0 count-2, f1/f2 count, f3 per-stage;
/// D1/D2 = dense residual stream WITHOUT mask words — empty).
#[cfg(test)]
pub(crate) fn extract_mask_segments(
    model: crate::Model,
    rate: u32,
    samples: &[i16],
) -> Vec<Vec<u32>> {
    let payload = crate::serialize(model, rate, samples).expect("serialize");
    let count = samples.len();
    let mut pos = 14usize;
    // count varint
    pos += varint_len(count as u64);
    // param varint
    if model.kind == MODEL_PERIOD || model.kind == MODEL_OSC || model.kind == MODEL_OP {
        pos += varint_len(model.param as u64);
    }
    read_flat_masks(model, &payload, &mut pos, count)
}

/// Continue the walk at `pos` into the mask segments (post-header).
#[cfg(test)]
fn read_flat_masks(
    model: crate::Model,
    payload: &[u8],
    pos: &mut usize,
    count: usize,
) -> Vec<Vec<u32>> {
    match model.kind {
        MODEL_PERIOD => {
            let words = (count - model.param).div_ceil(32);
            vec![flat(payload, pos, words)]
        }
        MODEL_OSC => {
            let family = crate::osc_variant_family(model.param);
            let grid = crate::osc_variant_grid(model.param);
            match family {
                0 => {
                    *pos += 4; // y0/y1 seeds
                    *pos += if grid == 2 {
                        vlen(payload, *pos) + vlen(payload, *pos + vlen(payload, *pos))
                    } else {
                        8
                    };
                    vec![flat(payload, pos, (count - 2).div_ceil(32))]
                }
                1 => {
                    *pos += if grid == 2 {
                        vlen(payload, *pos) + vlen(payload, *pos + vlen(payload, *pos))
                    } else {
                        8
                    };
                    vec![flat(payload, pos, count.div_ceil(32))]
                }
                2 => {
                    *pos += 8; // amp + rq i32
                    for _ in 0..5 {
                        *pos += vlen(payload, *pos);
                    }
                    vec![flat(payload, pos, count.div_ceil(32))]
                }
                _ => {
                    // f3: stage count + per-stage (c, r); one seg per stage
                    let n = vlen(payload, *pos);
                    let stages = vint_value(payload, *pos, n);
                    *pos += n;
                    for _ in 0..stages {
                        *pos += vlen(payload, *pos);
                        *pos += vlen(payload, *pos);
                    }
                    let seg_words = count.div_ceil(32);
                    (0..stages)
                        .map(|k| flat(payload, &mut (*pos + k * seg_words * 4), seg_words))
                        .collect()
                }
            }
        }
        MODEL_OP => {
            // OP masks are ALREADY RLE on the wire — the M-arms are moot here
            // (G9: 3 B on silence); no flat words to re-encode.
            Vec::new()
        }
        _ => Vec::new(), // RAW/D1/D2: no mask words on the wire
    }
}

#[cfg(test)]
fn vint_value(payload: &[u8], at: usize, n: usize) -> usize {
    let mut v = 0u64;
    let mut shift = 0;
    for &b in payload[at..at + n].iter() {
        v |= ((b & 0x7F) as u64) << shift;
        shift += 7;
    }
    v as usize
}

#[cfg(test)]
fn flat(payload: &[u8], pos: &mut usize, words: usize) -> Vec<u32> {
    let b = payload[*pos..*pos + words * 4].to_vec();
    *pos += words * 4;
    b.chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[cfg(test)]
fn vlen(payload: &[u8], at: usize) -> usize {
    let mut n = 0usize;
    loop {
        let b = payload[at + n];
        n += 1;
        if b & 0x80 == 0 {
            break;
        }
    }
    n
}

/// (residual bytes, mask bytes, zigzag symbols) for a chosen model's payload —
/// the G11 audit's inputs, walked from the same production layout.
#[cfg(test)]
pub(crate) fn ledger_bits(
    model: crate::Model,
    rate: u32,
    samples: &[i16],
) -> (usize, usize, Vec<u64>) {
    let (spans, payload) = ledger(model, rate, samples);
    let mut res = 0usize;
    let mut maskb = 0usize;
    for s in &spans {
        match s.category {
            "residuals" => res += s.end - s.begin,
            "masks" => maskb += s.end - s.begin,
            _ => {}
        }
    }
    // symbols: the delta varint stream — reuse decode-side knowledge: for the
    // OSC kinds the symbols are the residual span's varints; lazily the
    // caller may pass the plan deltas. For the audit we re-extract by
    // reading the payload's residual span as varints.
    (res, maskb, read_residual_symbols(&payload, spans, model))
}

/// Read the zigzag symbols inside the payload's residual span(s).
#[cfg(test)]
fn read_residual_symbols(payload: &[u8], spans: Vec<Span>, _model: crate::Model) -> Vec<u64> {
    let mut syms = Vec::new();
    for s in spans {
        if s.category == "residuals" {
            let mut p = s.begin;
            while p < s.end {
                let mut n = 0usize;
                loop {
                    let b = payload[p + n];
                    n += 1;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                let mut v = 0u64;
                let mut shift = 0;
                for &b in payload[p..p + n].iter() {
                    v |= ((b & 0x7F) as u64) << shift;
                    shift += 7;
                }
                syms.push(v);
                p += n;
            }
        }
    }
    syms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Vec<(&'static str, u32, Vec<Vec<i16>>)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        ["example_word.wav", "can_you_stop.wav", "sample2.wav"]
            .iter()
            .filter_map(|name| {
                let data = std::fs::read(format!("{dir}/{name}")).ok()?;
                let (rate, streams) = crate::parse_wav_multichannel(&data).ok()?;
                Some((*name, rate, streams))
            })
            .collect()
    }

    fn fixture_variants() -> Vec<(&'static str, Vec<i16>)> {
        vec![
            (
                "two-tone",
                (0..4096usize)
                    .map(|i| {
                        let t = i as f64 / 44100.0;
                        (8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                            + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos())
                        .round()
                        .clamp(-32767.0, 32767.0) as i16
                    })
                    .collect(),
            ),
            ("silence", vec![0i16; 4096]),
        ]
    }

    fn anatomy(name: &str, model: crate::Model, rate: u32, y: &[i16], rows: &mut String) {
        let (spans, payload) = ledger(model, rate, y);
        // the review's five checks
        let covered: usize = spans.iter().map(|s| s.end - s.begin).sum();
        assert_eq!(covered, payload.len(), "{name}: bytes unaccounted");
        let mut sorted: Vec<&Span> = spans.iter().collect();
        sorted.sort_by_key(|s| s.begin);
        let mut cursor = 0usize;
        for s in &sorted {
            assert_eq!(s.begin, cursor, "{name}: gap at {}", s.begin);
            cursor = s.end;
            assert!(s.end <= payload.len(), "{name}: span past payload end");
        }
        for s in &spans {
            assert!(
                matches!(
                    s.category,
                    "metadata"
                        | "masks"
                        | "residuals"
                        | "seeds"
                        | "segmentation"
                        | "coefficients"
                        | "other"
                ),
                "unknown category {}",
                s.category
            );
        }
        let mut by_cat: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in &spans {
            *by_cat.entry(s.category).or_insert(0) += s.end - s.begin;
        }
        let cat = |c: &str| by_cat.get(c).copied().unwrap_or(0);
        *rows += &format!(
            "G9 {name} (kind={} param={}): total={}B | metadata={} masks={} residuals={} seeds={} coefficients={} other={}\n",
            model.kind, model.param, payload.len(),
            cat("metadata"), cat("masks"), cat("residuals"), cat("seeds"), cat("coefficients"), cat("other"),
        );
    }

    /// The G9 contract: five invariant checks everywhere + the anatomy table.
    #[test]
    fn g9_bitstream_anatomy() {
        let mut rows = String::new();
        for (name, rate, streams) in corpus() {
            for (ch, y) in streams.iter().enumerate() {
                let model = choose_model(y);
                anatomy(&format!("{name} ch{ch}"), model, rate, y, &mut rows);
            }
        }
        for (name, y) in fixture_variants() {
            let model = choose_model(&y);
            anatomy(name, model, 44100, &y, &mut rows);
        }
        eprintln!("{rows}");
    }
}
