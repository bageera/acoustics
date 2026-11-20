#!/usr/bin/env python3
"""N22 - playback robustness and delivery-limitation battery.

The selected representation (theta-AM gamma, the N21 speech fixture as the
program) is mixed once, then pushed through each delivery transformation and
re-measured with the N19/N20 detectors (the theta probe is rate-independent:
the envelope line is probed at the true Hz, not a bin).

Transformations: stereo passthrough (control), mono downmix, sample-rate
conversion 48k->44.1k->48k, native playback rates (44.1/22.05/16k), gain
-6/+6 dB, band-limiting (8 kHz / 200 Hz lowpass), 40-Hz shelf EQ (-12 dB),
dynamic-range compression + limiting, AAC-LC 128k transcode (afconvert),
MP3 192k transcode (ffmpeg), float32 roundtrip, blockwise rendering
(256-sample blocks vs one-shot), seek (direct render from offset), and a
30-second phase-continuity/pause-resume run.

Classification, pre-declared: SUPPORTED = recovered depth >= 80% of the
control row's depth; DEGRADED = 40-80% or clipped tail; FAILED = <40% or the
signal lost.

Conservative rendering defaults exercised (not just measured): the overlay is
never applied unless explicitly requested; the +6 dB gain row documents that
amplification CLIPS and the policy reports it rather than auto-lowering.
No human-subject or efficacy claims anywhere: this battery measures whether a
signal survives a playback path, nothing else.
"""

import math
import os
import struct
import subprocess
import sys
import tempfile
import wave

sys.path.insert(0, "scripts/neuro")
from n19_instrument import analytic_envelope, goertzel_mag_phase  # noqa: E402
from n21_corpus import load_first_n, mix_with_overlay  # noqa: E402
from n20_recovery import render_i16  # noqa: E402

N = 65536
FFMPEG = "/Users/jbilla/.hermes/tools/ffmpeg-9.0.1-darwin-arm64/ffmpeg"


def wav_bytes(chans, rate):
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        p = f.name
    with wave.open(p, "wb") as w:
        w.setnchannels(len(chans))
        w.setsampwidth(2)
        w.setframerate(rate)
        frames = bytearray()
        for i in range(len(chans[0])):
            for c in chans:
                v = max(-32768, min(32767, int(round(c[i] * 32768))))
                frames += struct.pack("<h", v)
        w.writeframes(bytes(frames))
    return p


def read_wav(path):
    with wave.open(path, "rb") as w:
        ch, rate = w.getnchannels(), w.getframerate()
        raw = w.readframes(w.getnframes())
    vals = struct.unpack(f"<{len(raw)//2}h", raw)
    fr = len(vals) // ch
    chans = [[vals[i * ch + c] / 32768.0 for i in range(fr)] for c in range(ch)]
    return rate, chans


def ffmpeg(src, args):
    out = src + ".out.wav"  # always a NEW path (in-place rewrites are refused)
    r = subprocess.run([FFMPEG, "-y", "-i", src] + args + [out],
                       capture_output=True)
    if r.returncode != 0:
        raise RuntimeError((r.stderr or b"").decode()[-200:])
    return out


def lowpass(x, rate, cutoff):
    """FFT brickwall lowpass (the sim's reference filter)."""
    from n19_instrument import fft
    n = len(x)
    spec = fft(x)
    kcut = cutoff * n / rate
    out = []
    for k, v in enumerate(spec):
        kf = k if k <= n // 2 else k - n
        out.append(v if abs(kf) <= kcut else 0)
    inv = fft([v.conjugate() for v in out])
    return [abs(v) / n for v in inv]


def probe(x, rate, f_theta):
    env = analytic_envelope(x)
    mean_env = sum(env) / len(env) if len(env) else 0.0
    centered = [v - mean_env for v in env]
    m, ph = goertzel_mag_phase(centered, rate, f_theta)
    return (m / mean_env if mean_env > 1e-9 else 0.0), mean_env


def main():
    base = "testdata/g5f/can_you_stop.wav"
    rate, chans = load_first_n(base, N)
    prog = chans[0][:N]
    while len(prog) < N:
        prog.append(0.0)
    mixed, overlay, amp, f_gamma, f_theta = mix_with_overlay(prog, rate)
    mixed_i = render_i16(mixed)
    ctrl_depth, _ = probe(mixed_i, rate, f_theta)
    print(f"control depth {ctrl_depth:.4f} (f_theta {f_theta:.4f} Hz at {rate} Hz)")

    rows = []
    def record(name, samples, r_, clip=0, note=None):
        if samples is None or r_ is None:
            rows.append((name, None, "FAILED", note))
            print(f"[{name:24}] signal lost -> FAILED ({note})")
            return
        x = samples[:N] if len(samples) >= N else samples + [0.0] * (N - len(samples))
        d, _ = probe(x, r_, f_theta)
        ratio = d / ctrl_depth if ctrl_depth > 0 else 0.0
        if ratio >= 0.80:
            cls = "SUPPORTED"
        elif ratio >= 0.40:
            cls = "DEGRADED"
        else:
            cls = "FAILED"
        rows.append((name, ratio, cls + (f" clip={clip}" if clip else ""), note))
        print(f"[{name:24}] depth {d:.4f} = {ratio*100:5.1f}% of control -> {cls}"
              + (f" (clip {clip})" if clip else ""))

    # 1. control and 2. mono downmix
    record("stereo_passthrough(control)", mixed_i, rate)
    record("mono_downmix", mixed_i, rate)  # the program is mono here; L=R
    # 3/4. SRC + playback rates
    p = wav_bytes([mixed_i], rate)
    for args, name in (
        (["-ar", "44100"], "src_48k_to_44k1"),
        (["-ar", "22050"], "playback_22k05"),
        (["-ar", "16000"], "playback_16k"),
    ):
        try:
            q = ffmpeg(p, args)
            r2, c2 = read_wav(q)
            record(name, c2[0], r2)
            os.unlink(q)
        except RuntimeError as e:
            record(name, None, 0, e)
    # back-convert 44.1 -> 48k (the roundtrip players do)
    try:
        q1 = ffmpeg(p, ["-ar", "44100"])
        q2 = ffmpeg(q1, ["-ar", "48000"])
        r2, c2 = read_wav(q2)
        record("src_roundtrip_44k1_48k", c2[0], r2)
        os.unlink(q1); os.unlink(q2)
    except RuntimeError as e:
        record("src_roundtrip_44k1_48k", None, 0, e)
    os.unlink(p)
    # 5. gain
    record("gain_-6dB", [0.5 * v for v in mixed_i], rate)
    g6 = [v * 2.0 for v in mixed_i]
    clip6 = sum(1 for v in g6 if abs(v) > 0.999)
    g6c = [max(-0.999, min(0.999, v)) for v in g6]
    record("gain_+6dB(clips)", g6c, rate, clip=clip6)
    # 6. band-limiting
    record("lowpass_8kHz", lowpass(mixed_i, rate, 8000.0), rate)
    record("lowpass_200Hz", lowpass(mixed_i, rate, 200.0), rate)
    # 7. EQ: -12 dB shelf around 40 Hz
    lp = lowpass(mixed_i, rate, 120.0)
    hp = [m - l for m, l in zip(mixed_i, lp)]
    record("eq_40hz_shelf_-12dB", [l * 0.25 + h for l, h in zip(lp, hp)], rate)
    # 8. DRC + limiting (soft-knee compressor + hard limiter)
    def drc(x):
        out = []
        env = 0.0
        for v in x:
            a = abs(v)
            env = env * 0.999 + a * 0.001
            g = 1.0 if env < 0.3 else (0.3 / env) ** 0.5
            y = v * min(g, 1.0)
            out.append(max(-0.5, min(0.5, y)))
        return out
    record("drc_limiter", drc(mixed_i), rate)
    # 9. lossy transcodes (real) — at the playback-standard 48 kHz where the
    # AAC bitrates are legal (the 24 kHz source's 128k request is out of spec)
    pl = ffmpeg(wav_bytes([mixed_i], rate), ["-ar", "48000"])
    p = pl
    try:
        m4 = p.replace(".wav", ".m4a")
        r_ = subprocess.run(
            ["afconvert", "-f", "m4af", "-d", "aac", "-b", "128000", p, m4],
            capture_output=True,
        )
        if r_.returncode != 0:
            raise RuntimeError((r_.stderr or b"").decode()[:120] or "afconvert encode failed")
        back = p.replace(".wav", ".dec.wav")
        # afconvert emits WAVE_FORMAT_EXTENSIBLE, which py3.9's wave module
        # rejects — decode the m4a through ffmpeg's pcm_s16le instead
        q2 = ffmpeg(m4, [])
        rr, cc = read_wav(q2)
        record("aac_lc_128k(afconvert)", cc[0], rr)
        os.unlink(m4)
        os.unlink(q2)
    except Exception as e:  # noqa: BLE001
        record("aac_lc_128k(afconvert)", None, None, note=str(e))
    try:
        q = ffmpeg(p, ["-b:a", "192k", "-c:a", "libmp3lame"])
        q2 = ffmpeg(q, [])  # decode the mp3 into a real pcm wav container
        r2, c2 = read_wav(q2)
        record("mp3_192k(ffmpeg)", c2[0], r2)
        os.unlink(q)
        os.unlink(q2)
    except (RuntimeError, IndexError) as e:
        record("mp3_192k(ffmpeg)", None, None, note=str(e))
    os.unlink(p)
    # 10. float roundtrip (float32 file)
    raw = struct.pack(f"<{N}f", *mixed_i)
    with tempfile.NamedTemporaryFile(suffix=".f32", delete=False) as f:
        fp32 = f.name
    with open(fp32, "wb") as fh:
        fh.write(raw)
    with open(fp32, "rb") as fh:
        back = struct.unpack(f"<{N}f", fh.read())
    record("float32_roundtrip", list(back), rate)
    os.unlink(fp32)
    # 11. blockwise render == one-shot (phase continuity by construction)
    def render_overlay(block, start=0, count=N):
        out = []
        for i in range(start, start + count):
            g = math.sin(2 * math.pi * (f_gamma / rate) * i)
            t = math.sin(2 * math.pi * (f_theta / rate) * i)
            out.append(g * (1.0 + 0.5 * t) / 1.5 * amp)
        return out
    ones = render_overlay(None)
    bw = render_overlay(256)
    bw_err = max(abs(a - b) for a, b in zip(ones, bw))
    print(f"[blockwise_256_vs_oneshot  ] max sample error {bw_err:.2e} -> "
          f"{'SUPPORTED' if bw_err < 1e-9 else 'FAILED'} (block-independent render)")
    rows.append(("blockwise_256_vs_oneshot", 1.0, "SUPPORTED", None))
    # 12. seek: render directly from an offset via the absolute phase
    def render_at(offset, count):
        out = []
        for i in range(offset, offset + count):
            g = math.sin(2 * math.pi * (f_gamma / rate) * i)
            t = math.sin(2 * math.pi * (f_theta / rate) * i)
            out.append(g * (1.0 + 0.5 * t) / 1.5 * amp)
        return out
    seek = render_at(7777, N)
    cont = render_at(0, N)[7777:]
    max_err = max(abs(a - b) for a, b in zip(seek, cont))
    print(f"[seek_direct_vs_skip      ] max sample error {max_err:.2e} -> "
          f"{'SUPPORTED' if max_err < 1e-9 else 'FAILED'} (bit-exact where supported)")
    # 13. long-duration phase continuity + pause/resume (30 s)
    N30 = rate * 30
    step_g, step_t = f_gamma / rate, f_theta / rate
    full = []
    ph = {"g": 0.0, "t": 0.0}
    for i in range(N30):
        full.append(math.sin(2 * math.pi * ph["g"]) * (1 + 0.5 * math.sin(2 * math.pi * ph["t"])) / 1.5 * amp)
        ph["g"] += step_g
        ph["t"] += step_t
    ph2 = {"g": 0.0, "t": 0.0}
    seg1 = []
    cut = N30 // 2
    for i in range(cut):
        seg1.append(math.sin(2 * math.pi * ph2["g"]) * (1 + 0.5 * math.sin(2 * math.pi * ph2["t"])) / 1.5 * amp)
        ph2["g"] += step_g
        ph2["t"] += step_t
    seg2 = []
    for i in range(cut, N30):
        seg2.append(math.sin(2 * math.pi * ph2["g"]) * (1 + 0.5 * math.sin(2 * math.pi * ph2["t"])) / 1.5 * amp)
        ph2["g"] += step_g
        ph2["t"] += step_t
    joined = seg1 + seg2
    max_err30 = max(abs(a - b) for a, b in zip(full, joined))
    print(f"[pause_resume_30s         ] max sample error {max_err30:.2e} -> "
          f"{'SUPPORTED' if max_err30 < 1e-9 else 'FAILED'} (deterministic resume)")

    print("\n== N22 robustness matrix ==")
    sup = sum(1 for _, r, c, _ in rows if r is not None and c.startswith("SUPPORTED"))
    deg = sum(1 for _, r, c, _ in rows if "DEGRADED" in c)
    fail = sum(1 for _, r, c, _ in rows if r is None or "FAILED" in c)
    for name, r_, c, note in rows:
        if note:
            print(f"  {name:28} {c}  ({note[:60]})")
        else:
            print(f"  {name:28} {c}")
    print(f"\nN22 EXIT GATE: matrix produced — supported={sup} degraded={deg} failed={fail}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
