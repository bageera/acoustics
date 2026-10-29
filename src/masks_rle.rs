use crate::{read_varint, varint_len, write_varint, ERR_TRUNCATED};
/// Mask-word RLE (v0.7): masks dominated all-zero payloads (v0.4 P5 measured
/// the 4 B-per-32-samples floor as the structural cost of exact fits — an
/// all-zero mask table for 5,152 samples cost 644 B). Wire: varint(run_count),
/// then per run varint(count) with runs ALTERNATING zero-run/data-run starting
/// with zero-run; a data run's raw u32 words follow its count.
pub(crate) fn mask_rle_wire_len(masks: &[u32]) -> usize {
    // zero-run-first: when the table begins with a data run the writer prepends a
    // 0-length zero run (+1 run, +1 count varint).
    let mut runs = 0usize;
    let mut count_bytes = 0usize;
    let mut data_words = 0usize;
    let mut i = 0usize;
    // zero-run-first: the writer prepends a 0-length zero run when the table begins
    // with a data run; count that varint too.
    if masks.first().is_some_and(|&m| m != 0) {
        count_bytes += varint_len(0);
    }
    while i < masks.len() {
        runs += 1;
        if masks[i] == 0 {
            let st = i;
            while i < masks.len() && masks[i] == 0 {
                i += 1;
            }
            count_bytes += varint_len((i - st) as u64);
        } else {
            let st = i;
            while i < masks.len() && masks[i] != 0 {
                i += 1;
            }
            count_bytes += varint_len((i - st) as u64);
            data_words += i - st;
        }
    }
    varint_len(runs as u64) + count_bytes + data_words * 4
}

pub(crate) fn write_mask_rle(masks: &[u32], out: &mut Vec<u8>) {
    let mut runs: Vec<(bool, usize, usize)> = Vec::new(); // (is_zero, start, count)
    let mut i = 0usize;
    while i < masks.len() {
        if masks[i] == 0 {
            let st = i;
            while i < masks.len() && masks[i] == 0 {
                i += 1;
            }
            runs.push((true, st, i - st));
        } else {
            let st = i;
            while i < masks.len() && masks[i] != 0 {
                i += 1;
            }
            runs.push((false, st, i - st));
        }
    }
    // canonical alternation: zero-run FIRST (a 0-length zero-run when the table
    // starts with set words) — the reader/wire-len assume zero-run-first.
    if let Some((false, _, _)) = runs.first() {
        runs.insert(0, (true, 0, 0));
    }
    write_varint(out, runs.len() as u64);
    for (is_zero, st, cnt) in runs {
        write_varint(out, cnt as u64);
        if !is_zero {
            for &m in &masks[st..st + cnt] {
                out.extend_from_slice(&m.to_le_bytes());
            }
        }
    }
}

pub(crate) fn read_mask_rle(
    payload: &[u8],
    pos: &mut usize,
    word_count: usize,
) -> Result<Vec<u32>, String> {
    let run_count = read_varint(payload, pos)? as usize;
    let mut masks = Vec::with_capacity(word_count);
    let mut zero_next = true;
    for _ in 0..run_count {
        let cnt = read_varint(payload, pos)? as usize;
        if !zero_next {
            for _ in 0..cnt {
                let b = payload.get(*pos..*pos + 4).ok_or(ERR_TRUNCATED)?;
                masks.push(u32::from_le_bytes(b.try_into().unwrap()));
                *pos += 4;
            }
        } else {
            masks.extend(std::iter::repeat_n(0u32, cnt));
        }
        zero_next = !zero_next; // runs alternate: zero-run, data-run, ...
    }
    if masks.len() != word_count {
        return Err("OP mask run count mismatch".into());
    }
    Ok(masks)
}
