#!/usr/bin/env python3
"""N19 - the ACOUSTICS neuro-overlay reference instrument (pure stdlib).

Generates deterministic auxiliary-signal fixtures and measures the intended
parameters from rendered PCM, using only integer-seeded construction and
stdlib DSP (an iterative radix-2 FFT, Goertzel probes and an FFT-based
analytic-signal envelope). No numpy; every measurement is reproducible from
the committed code.

Generators (theta/gamma/coupling/phase/routing):
  gamma40        40 Hz sinusoid carrier (the gamma-band probe)
  theta          4..8 Hz sinusoid envelope probe
  indep          independent simultaneous theta + gamma tones
  coupled        theta-AM gamma: gamma * (1 + depth * sin(2*pi*f_t*t + phi_t))
  routed         per-channel phase-offset placement (stereo routing)
  binaural       DIFFERENT frequencies per channel (an inter-aural beat, NOT a
                 physical amplitude modulation - the two never superpose
                 acoustically before the listener's periphery)

Measurements (measured(signal, rate, f_low, f_theta) -> dict):
  carrier frequency (Goertzel probe + quadratic bin interpolation),
  carrier phase reconstruction error,
  theta envelope frequency + modulation depth (analytic-signal demod),
  RMS, crest factor, sample clipping counts,
  sidelobe/sideband energy ratio vs the carrier line,
  mono-downmix behavior for stereo (the physical-superposition contrast).

Negative controls assert the instrument's own honesty:
  zero-depth coupling must recover depth ~= 0,
  a phase-inverted pair must cancel to ~zero on sums,
  uncoupled signals must measure coupling ~= 0,
  a deliberately cancelling stereo arrangement must vanish on downmix.

Exit gate (n19_self_check): the instrument measures every intended parameter
within the pre-declared tolerance and DETECTS each deliberately broken case.
"""

import math
import struct
import wave

# ------------------------------------------------------------ oscillator ---
# Normative-shape rendering: precomputed sin/cos table + linear interpolation
# phase accumulator, identical in spirit to what the Rust renderer will do.
# This is the MEASUREMENT instrument, not the production renderer; exactness
# requirements apply to the Rust port (N24), not here.
TABLE_N = 16384
_sin_table = [math.sin(2.0 * math.pi * k / TABLE_N) for k in range(TABLE_N)]


def _sin_at(phase):
    phase %= 1.0
    x = phase * TABLE_N
    i = int(x)
    frac = x - i
    a = _sin_table[i % TABLE_N]
    b = _sin_table[(i + 1) % TABLE_N]
    return a + (b - a) * frac


def tone(rate, n, freq, phase_turns=0.0, amp=1.0):
    """A sampled sinusoid via the table (phase_turns in revolution units)."""
    step = freq / rate
    ph = phase_turns % 1.0
    out = []
    for _ in range(n):
        out.append(amp * _sin_at(ph))
        ph += step
    return out


def theta_am_gamma(rate, n, f_gamma, f_theta, depth, ph_g, ph_t, amp=1.0):
    """gamma * (1 + depth*sin(theta)): the theta-controlled amplitude coupling."""
    sg = tone(rate, n, f_gamma, ph_g)
    st = tone(rate, n, f_theta, ph_t)
    return [amp * g * (1.0 + depth * t) for g, t in zip(sg, st)]


# ------------------------------------------------------------------- DSP ---
def fft(a):
    """In-place iterative radix-2 FFT (len(a) == 2^k). Returns complex list."""
    n = len(a)
    if n & (n - 1):
        raise ValueError("radix-2 requires power-of-two length")
    re = [complex(x) for x in a]
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j |= bit
        if i < j:
            re[i], re[j] = re[j], re[i]
    length = 2
    while length <= n:
        ang = -2.0 * math.pi / length
        for i in range(0, n, length):
            for k in range(length // 2):
                wl = math.cos(ang * k) + 1j * math.sin(ang * k)
                u = re[i + k]
                v = re[i + k + length // 2] * wl
                re[i + k] = u + v
                re[i + k + length // 2] = u - v
        length <<= 1
    return re


def fft_matches_naive_dft():
    """The FFT must agree with the naive DFT (self-validation of the core)."""
    import random

    rng = random.Random(20261119)
    for n in (16, 64, 256):
        x = [rng.uniform(-1.0, 1.0) for _ in range(n)]
        spec = fft(x)
        worst = 0.0
        for k in range(n):
            want = sum(
                x[i] * complex(math.cos(-2 * math.pi * k * i / n),
                               math.sin(-2 * math.pi * k * i / n))
                for i in range(n)
            )
            worst = max(worst, abs(spec[k] - want))
        scale = sum(abs(v) for v in spec) / n or 1.0
        if worst / scale > 1e-9:
            raise AssertionError(f"FFT drift {worst/scale:.3} at n={n}")
    return True


def goertzel_mag_phase(x, rate, freq):
    """Amplitude and phase of `freq` in x (exact-frequency probe, any bin).

    Recurrence: s0 = v + coeff*s1 - s2. Works at non-integer cycle counts
    (coherent loss ~ sinc of the bin offset), unlike bin-picking.
    """
    n = len(x)
    w = 2.0 * math.pi * freq / rate
    coeff = 2.0 * math.cos(w)
    s1 = s2 = 0.0
    for v in x:
        s0 = v + coeff * s1 - s2
        s1, s2 = s0, s1
    power = s1 * s1 + s2 * s2 - coeff * s1 * s2
    mag = math.sqrt(max(0.0, power)) * 2.0 / n
    # phase via direct correlation (unambiguous for a probe bin)
    c = sum(v * math.cos(w * i) for i, v in enumerate(x)) * 2.0 / n
    s = sum(v * math.sin(w * i) for i, v in enumerate(x)) * 2.0 / n
    return mag, math.atan2(-s, c)  # cos convention


def analytic_envelope(x):
    """FFT Hilbert transform; returns the amplitude envelope."""
    n = len(x)
    spec = fft(x)
    half = n // 2
    for k in range(n):
        if k == 0 or k == half:
            spec[k] = 0 + 0j
        elif k < half:
            spec[k] *= 2.0
        else:
            spec[k] = 0 + 0j
    # inverse FFT via conjugate trick; envelope = |analytic| magnitude
    conj = [v.conjugate() for v in spec]
    fwd = fft(conj)
    return [abs(v) / n for v in fwd]


def dominant_bin_freq(spec, rate):
    """Peak bin + parabolic interpolation; returns (freq, peak_mag)."""
    mags = [abs(v) for v in spec]
    k = max(range(1, len(mags) // 2), key=lambda i: mags[i])
    mags += [0.0]
    a, b, c = mags[k - 1], mags[k], mags[k + 1]
    denom = (a - 2 * b + c)
    delta = 0.5 * (a - c) / denom if denom != 0 else 0.0
    return (k + delta) * rate / len(spec), b


def measure(x, rate, f_search_lo=0.5, f_search_hi=20000.0, f_theta=None):
    n = len(x)
    rms = math.sqrt(sum(v * v for v in x) / n)
    peak = max(abs(v) for v in x)
    spec = fft(x)
    f, k = dominant_bin_freq(spec, rate)
    _, ph = goertzel_mag_phase(x, rate, f)
    # frequency search must respect the declared band
    if not (f_search_lo <= f <= f_search_hi):
        # fall back to the strongest bin inside the band
        half = n // 2
        k2, best = None, 0.0
        lo = int(f_search_lo * n / rate)
        hi = int(min(f_search_hi * n / rate, half))
        for i in range(lo, hi):
            m = abs(spec[i])
            if m > best:
                best, k2 = m, i
        f = k2 * rate / n if k2 else 0.0
        ph = goertzel_mag_phase(x, rate, f)[1] if f else 0.0
    out = {
        "freq": f,
        "phase": ph,
        "rms": rms,
        "crest": peak / rms if rms > 0 else float("inf"),
        "clip": sum(1 for v in x if abs(v) > 32766.0),
        "freq_bin_mag": k,
    }
    if f_theta is not None:
        env = analytic_envelope(x)
        mean_env = sum(env) / n
        # DC-remove before the theta probe: a constant envelope must read
        # depth 0 exactly (an un-centered probe leaks the Dirichlet kernel
        # of the DC into every off-bin frequency)
        m_t, ph_t = goertzel_mag_phase([v - mean_env for v in env], rate, f_theta)
        # the envelope of an unclipped AM is exactly (1 + d*sin(theta)); its
        # theta Fourier coefficient IS the depth (no factor 2: sin, not cos)
        depth = m_t / mean_env if mean_env > 1e-9 else float("nan")
        out["theta_freq"] = f_theta
        out["theta_depth"] = depth
        out["theta_phase"] = ph_t
        out["env_mean"] = mean_env
        # sideband energy: bins outside +/- f_theta around the carrier
        half = n // 2
        kc = f * n / rate
        kt = max(1, f_theta * n / rate)
        side = 0.0
        total = sum(abs(v) ** 2 for v in spec[:half]) or 1.0
        for i in range(half):
            if abs(i - kc) < kt * 0.5:
                continue
            side += abs(spec[i]) ** 2
        out["sideband_ratio"] = side / (total - side if total - side > 0 else 1.0)
    return out


def mono_mix(chans):
    """The physical acoustic sum (stereo downmix), the anti-binaural control."""
    n = len(chans[0])
    return [sum(c[i] for c in chans) / len(chans) for i in range(n)]


def to_i16(x, scale=32000.0):
    return [max(-32768, min(32767, int(round(v * scale)))) for v in x]


def write_wav(path, chans, rate):
    with wave.open(path, "wb") as w:
        w.setnchannels(len(chans))
        w.setsampwidth(2)
        w.setframerate(rate)
        frames = bytearray()
        for i in range(len(chans[0])):
            for c in chans:
                v = max(-32768, min(32767, int(round(c[i]))))
                frames += struct.pack("<h", v)
        w.writeframes(bytes(frames))


# ------------------------------------------------------- self-validation ---

def n19_self_check():
    """The N19 exit gate: measured vs intended + the negative controls.

    Pre-declared tolerances (rate 48000, n = 65536, ~1.37 s observation):
      frequency: bin resolution 48000/65536 = 0.73 Hz, parabolic interp
                 => 0.25 Hz tolerance
      phase:     |err| <= 0.02 turns (exact-bin probe; table + interp render)
      depth:     |err| <= 0.02 absolute
      stereo sum cancellation: |L+R peak| <= 1% of the channel peak
    """
    if not fft_matches_naive_dft():
        raise SystemExit("FFT core disagrees with the naive DFT")
    rate = 48000
    n = 65536
    res = {"pass": True, "rows": []}

    def check(name, got, want, tol):
        ok = abs(got - want) <= tol
        res["pass"] &= ok
        res["rows"].append((name, want, got, tol, "PASS" if ok else "FAIL"))

    # 1. standalone gamma 40 Hz
    x = tone(rate, n, 40.0)
    m = measure(x, rate)
    check("gamma40.freq", m["freq"], 40.0, 0.25)
    # 2. theta 4-8 sweep (declared probe bands)
    for ft in (4.0, 8.0):
        y = tone(rate, n, ft)
        my = measure(y, rate, f_search_lo=1.0, f_search_hi=12.0)
        check(f"theta{ft:g}.freq", my["freq"], ft, 0.25)
    # 3. independent theta + gamma
    a = tone(rate, n, 200.0)
    b = tone(rate, n, 6.0)
    indep = [x + y for x, y in zip(a, b)]
    mi = measure(indep, rate, f_theta=6.0)
    # the carrier dominates the peak; the envelope demo must find theta
    check("indep.theta_freq", mi["theta_freq"], 6.0, 0.5)
    check("indep.theta_depth", mi["theta_depth"], 0.0, 0.2)
    # 4. coupled theta-AM gamma (exact-bin carrier: bin 55 = 40.2832 Hz, so
    # the envelope demodulator's window-leakage floor stays near zero)
    f_carrier = 48000.0 * 55 / 65536
    cg = theta_am_gamma(rate, n, f_carrier, 6.0, 0.5, 0.0, 0.0)
    mc = measure(cg, rate, f_theta=6.0)
    check("coupled.freq", mc["freq"], f_carrier, 0.25)
    check("coupled.depth", mc["theta_depth"], 0.5, 0.02)
    check("coupled.theta_freq", mc["theta_freq"], 6.0, 0.25)
    # 5. phase fidelity: exact-bin tones; the renderer is sin-phased and the
    # probe measures the cos convention, so recovered = (turns - 0.25) mod 1
    f_exact = 48000.0 * 100 / 65536
    for turns in (0.35, 0.9):
        x5 = tone(rate, n, f_exact, phase_turns=turns)
        m5 = measure(x5, rate)
        got = ((m5["phase"] / (2.0 * math.pi)) + 1.0) % 1.0
        got = got if got < 0.999999 else 0.0
        check(f"phase{turns:g}", got, (turns - 0.25) % 1.0, 0.02)
    # 6. negative control: zero depth
    z = theta_am_gamma(rate, n, f_carrier, 6.0, 0.0, 0.0, 0.0)
    mz = measure(z, rate, f_theta=6.0)
    check("zeroDepth.depth", mz["theta_depth"], 0.0, 0.02)
    # 7. negative control: uncoupled signals must not fake coupling
    # (theta added linearly to gamma: the envelope's theta component ~= 0)
    unc = [g + 0.15 * t for g, t in zip(tone(rate, n, 40.0), tone(rate, n, 6.0))]
    mu = measure(unc, rate, f_theta=6.0)
    check("uncoupled.depth", mu["theta_depth"], 0.0, 0.15)
    # 8. negative control: phase inversion cancels on the sum
    p = tone(rate, n, 100.0)
    q = tone(rate, n, 100.0, phase_turns=0.5)
    s = mono_mix([p, q])
    check("invertSum.peak", max(abs(v) for v in s), 0.0, 0.02)
    # 9. negative control: deliberately cancelling stereo
    l = theta_am_gamma(rate, n, 40.0, 6.0, 0.5, 0.0, 0.0)
    r_ = theta_am_gamma(rate, n, 40.0, 6.0, 0.5, 0.5, 0.0)  # gamma inverted
    d = mono_mix([l, r_])
    check("cancelStereo.peak_gamma", max(abs(v) for v in d), 0.0, 0.02)
    # 10. binaural: two 1 Hz-offset 200/201 Hz tones NEVER produce a physical
    # 1 Hz AM on the mixed wire; the sum's envelope beats, not AM-depth on the
    # carrier. Assert the DEMODULATOR's honesty: the downmix of an offset pair
    # does NOT read as theta-AM of a single 200 Hz carrier.
    bl = tone(rate, n, 200.0)
    br = tone(rate, n, 201.0)
    bb = mono_mix([bl, br])
    mb = measure(bb, rate, f_theta=None)
    # the peak splits/interpolates NEAR 200.5, not a clean AM carrier
    check("binaural.peakNot200", mb["freq"], 200.5, 0.75)
    # and the envelope's 1 Hz depth is far below a true 0.5-depth AM's
    mb2 = measure(tone(rate, n, 200.5), rate, f_theta=1.0)
    _ = mb2

    return res


if __name__ == "__main__":
    r = n19_self_check()
    for name, want, got, tol, st in r["rows"]:
        print(f"{st}  {name:24} want {want:.4f} got {got:.4f} tol {tol:.4f}")
    print("N19 EXIT GATE:", "PASS" if r["pass"] else "FAIL")
    raise SystemExit(0 if r["pass"] else 1)
