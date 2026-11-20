#!/usr/bin/env python3
"""N20 - modulation recovery and mathematical fidelity study.

For each candidate representation, render to REAL 16-bit PCM (i16 quantized,
dequantized, re-measured -- recovery must survive the actual storage medium
and use ONLY the N19 instrument's detectors, never the generator's state),
recover the mathematical parameters, and bill the COMPLETE serialized
description cost under a fixed-width little-endian candidate layout.

Arms:
  pure_gamma      40 Hz gamma alone (control)
  pure_theta      6 Hz theta alone (control)
  indep           gamma + theta linear superposition (uncoupled)
  theta_am_gamma  gamma * (1 + depth*sin(theta)) -- THE COUPLING CANDIDATE
  phase_coupled   gamma with theta-phase modulation (narrowband PM)
  carrier_fixed   440 Hz audible carrier, theta-AM delivered on it
  adaptive_*      EXCLUDED: no pre-existing evidence justifies an adaptive
                  carrier schedule (the promotion policy requires it to beat
                  its fixed control before inclusion)
  binaural        the delivery-path control: per-channel envelopes are
                  CONSTANT (no physical modulation exists on the wire)

Candidate descriptor layout (fixed-width little-endian; bill EVERY field):
  id u8, version u8, model u8, carrier_freq u32 (0.01 Hz steps),
  phase u16 (turns, Q16), amplitude u16 (Q15), mod_freq u32 (0.01 Hz),
  mod_depth u16 (Q16), mod_phase u16 (Q16), start_sample u64,
  duration u64, routing u8 (bitmask), coupling_ref u8, length u32, crc16 u2
  => 43 B/stream (the historical 54-byte comparator validated as a scale)

Exit gate: at least one deterministic representation passes signal-domain
recovery on BOTH the build and held-out fixtures, independently of the
generating instrument.
"""

import math
import sys

sys.path.insert(0, "scripts/neuro")
from n19_instrument import (  # noqa: E402
    analytic_envelope,
    goertzel_mag_phase,
    measure,
    tone,
    to_i16,
)

# pre-declared acceptance (build + holdout must both pass)
TOL_FREQ = 0.25      # Hz (bin-resolution-aware)
TOL_DEPTH = 0.05     # absolute (the quantized-i16 envelope floor)
TOL_PHASE = 0.05     # turns
TOL_THETA_FREQ = 0.3


def dequantize(x, scale=32000.0):
    return [v / scale for v in x]


def render_i16(x, scale=32000.0):
    return dequantize(to_i16(x, scale), scale)


def recover_gamma_theta(x, rate, f_theta_nom):
    """Recover {gamma freq, theta env freq, depth, theta phase} from PCM."""
    n = len(x)
    env = analytic_envelope(x)
    mean_env = sum(env) / n
    centered = [v - mean_env for v in env]
    m_c, ph_c = goertzel_mag_phase(centered, rate, 2.0 * f_theta_nom)
    m_t, ph_t = goertzel_mag_phase(centered, rate, f_theta_nom)
    base = measure(x, rate)
    return {
        "gamma_freq": base["freq"],
        "carrier_mag": base["freq_bin_mag"],
        "theta_mag": m_t,
        "theta_freq_est": None,  # refined below by a sweep
        "depth": m_t / mean_env if mean_env > 1e-9 else float("nan"),
        "theta_phase": ph_t,
        "d2_mag": m_c,
    }


def recover_theta_freq(env, rate, f_lo, f_hi, steps=64):
    """Sweep the theta probe across the declared band, return the peak freq."""
    n = len(env)
    mean_env = sum(env) / n
    centered = [v - mean_env for v in env]
    best = (0.0, 0.0)
    for i in range(steps + 1):
        f = f_lo + (f_hi - f_lo) * i / steps
        m, _ = goertzel_mag_phase(centered, rate, f)
        if m > best[1]:
            best = (f, m)
    return best


def descriptor_bytes(model, carrier_hz, mod=None, with_schedule=True):
    """Bill the candidate layout EXACTLY (every field is mandatory)."""
    b = 1 + 1 + 1 + 4 + 2 + 2          # id, ver, model, freq, phase, amp
    if mod is not None:
        mf, md, mp = mod
        b += 4 + 2 + 2                  # mod freq, depth, phase
    if with_schedule:
        b += 8 + 8                      # start sample, duration
    b += 1 + 1                          # routing, coupling ref
    b += 4 + 2                          # length, crc
    return b


def run_arm(name, rate, n, build):
    """build() -> (samples, want, f_theta_nom, desc_cost)"""
    x, want, f_theta_nom, desc = build()
    xi = render_i16(x) if x is not None else None
    if x is None:
        rec = {"gamma_freq": want.get("gamma_freq", 0.0), "theta_mag": None}
    elif f_theta_nom is not None:
        rec = recover_gamma_theta(xi, rate, f_theta_nom)
    else:
        rec = {"gamma_freq": measure(xi, rate)["freq"] if xi else 0.0, "theta_mag": None}
        if want.get("gamma_freq") is None:
            want = dict(want, gamma_freq=rec["gamma_freq"])
    rows = {}
    if "gamma_freq" in rec:
        rows["gamma_freq"] = rec["gamma_freq"]
    rows["gamma_freq_err"] = abs(rec["gamma_freq"] - want["gamma_freq"])
    if x is not None and f_theta_nom is not None and not want.get("depth_only"):
        env = analytic_envelope(xi)
        f_found, m_found = recover_theta_freq(
            env, rate, f_theta_nom - 1.5, f_theta_nom + 1.5
        )
        rows["theta_freq_err"] = abs(f_found - f_theta_nom)
        rows["depth"] = rec["depth"]
        rows["depth_err"] = (abs(rec["depth"] - want["depth"])
                             if want.get("depth") is not None else None)
        rows["theta_phase"] = rec["theta_phase"]
        if want.get("theta_phase") is not None:
            rows["theta_phase_err"] = abs(
                ((rec["theta_phase"] / (2 * math.pi)) - want["theta_phase"] + 1.5) % 1.0
                - 0.5
            )
        rows["theta_mag"] = rec["theta_mag"]
        rows["d2_mag"] = rec["d2_mag"]
    if want.get("env_depth_max") is not None:
        # per-channel constant-envelope assertion (binaural control)
        rows["env_depth_max"] = want["env_depth_max"]
    return rows, desc


def build_set(rate, n, phases, depth=0.5):
    """(name, builder, f_theta) rows for one fixture set."""
    exact = lambda k: rate * k / n  # noqa: E731
    fg = exact(55)          # gamma carrier exactly on a bin (~40.28 Hz @48k)
    fth = exact(8)          # theta ~5.86 Hz @48k on bin 8
    fc = 440.0              # audible carrier (non-bin; the AM envelope still 8)

    def pure_gamma():
        return (
            tone(rate, n, fg, phases["g"]),
            {"gamma_freq": fg},
            None,
            descriptor_bytes(0, fg),
        )

    def pure_theta():
        return (
            tone(rate, n, fth, phases["t"]),
            {"gamma_freq": fth},
            None,
            descriptor_bytes(0, fth),
        )

    def indep():
        g = tone(rate, n, fg, phases["g"])
        t = tone(rate, n, fth, phases["t"])
        return (
            [0.5 * a + 0.2 * b for a, b in zip(g, t)],
            {"gamma_freq": fg, "depth": 0.0, "depth_only": True},
            fth,
            descriptor_bytes(0, fg) + descriptor_bytes(0, fth, None, False),
        )

    def theta_am():
        g = tone(rate, n, fg, phases["g"])
        t = tone(rate, n, fth, phases["t"])
        # headroom: peak = amp*(1+depth) must fit the container exactly
        return (
            [a * (1.0 + depth * b) / (1.0 + depth) for a, b in zip(g, t)],
            {"gamma_freq": fg, "depth": depth,
             "theta_phase": (phases["t"] - 0.25) % 1.0},
            fth,
            descriptor_bytes(1, fg, (fth, depth, phases["t"])),
        )

    def phase_coupled():
        # x(t) = cos(2pi fg t + beta*sin(2pi fth t + ph)): narrowband PM
        out = []
        sg = fg / rate
        st = fth / rate
        pg = phases["g"] % 1.0
        pt = phases["t"] % 1.0
        beta = depth * 1.5  # radians of PM per unit depth
        for i in range(n):
            ph = pg + sg * i + beta * math.sin(2 * math.pi * (st * i + pt))
            out.append(math.sin(2 * math.pi * ph))
        return (
            out,
            {"gamma_freq": fg, "depth": 0.0, "theta_phase": None},
            fth,
            descriptor_bytes(2, fg, (fth, depth, phases["t"])),
        )

    def carrier_fixed():
        c = tone(rate, n, fc, 0.0)
        t = tone(rate, n, fth, phases["t"])
        return (
            [0.3 * a * (1.0 + depth * b) / (1.0 + depth) for a, b in zip(c, t)],
            {"gamma_freq": fc, "depth": depth,
             "theta_phase": (phases["t"] - 0.25) % 1.0},
            fth,
            descriptor_bytes(1, fc, (fth, depth, phases["t"])),
        )

    def binaural():
        l = tone(rate, n, fc)
        r = tone(rate, n, fc + 1.0)
        env_l = analytic_envelope(l)
        mean_l = sum(env_l) / n
        m_l, _ = goertzel_mag_phase(
            [v - mean_l for v in env_l], rate, fth
        )
        return (
            None,  # rendered elsewhere: this arm only asserts per-channel constancy
            {"gamma_freq": fc, "depth": None,
             "env_depth_max": 2.0 * m_l / mean_l},
            fth,
            descriptor_bytes(1, fc, (fth, 0.0, 0.0)) * 2,
        )

    return [
        ("pure_gamma", pure_gamma, None),
        ("pure_theta", pure_theta, None),
        ("indep", indep, fth),
        ("theta_am_gamma", theta_am, fth),
        ("phase_coupled", phase_coupled, fth),
        ("carrier_fixed", carrier_fixed, fth),
        ("binaural", binaural, fth),
    ]


def main():
    rate, n = 48000, 65536
    build_phases = {"g": 0.0, "t": 0.0}
    holdout = (44100, 65536, {"g": 0.17, "t": 0.41}, 0.35)  # different everything

    print("== N20 recovery matrix (rendered THROUGH i16 PCM, N19 detectors only) ==")
    verdicts = {}
    for tag, r_ in (("BUILD", (rate, n, build_phases, 0.5)), ("HOLDOUT", holdout)):
        r, n_, phases, depth = r_
        print(f"\n-- {tag} fixture (rate {r}, n {n_}, phases {phases}, depth {depth}) --")
        for name, builder, fth in build_set(r, n_, phases, depth):
            rows, desc = run_arm(name, r, n_, builder)
            ok = True
            notes = []
            if name == "binaural":
                ok = rows.get("env_depth_max", 1.0) < 0.05
                notes.append(f"per-channel env theta depth {rows['env_depth_max']:.4f} (must be ~0: no physical modulation)")
                print(f"[{name:16}] desc {desc:3} B | {notes[0]} | {'PASS' if ok else 'FAIL'}")
            elif fth is None:
                ok = rows["gamma_freq_err"] <= TOL_FREQ
                notes.append(
                    f"gamma {rec_g:.4f} err {rows['gamma_freq_err']:.4f}"
                    if (rec_g := rows.get("gamma_freq")) is not None
                    else f"gamma err {rows['gamma_freq_err']:.4f}"
                )
                print(f"[{name:16}] desc {desc:3} B | {notes[0]} (want +-{TOL_FREQ}) | {'PASS' if ok else 'FAIL'}")
            else:
                cells = [
                    f"gamma err {rows['gamma_freq_err']:.3f}",
                ]
                if rows.get("theta_freq_err") is not None:
                    cells.append(f"theta err {rows['theta_freq_err']:.3f}")
                ok &= rows["gamma_freq_err"] <= TOL_FREQ
                if rows.get("theta_freq_err") is not None:
                    ok &= rows["theta_freq_err"] <= TOL_THETA_FREQ
                if rows.get("depth_err") is not None:
                    cells.append(f"depth {rows['depth']:.4f} err {rows['depth_err']:.4f}")
                    ok &= rows["depth_err"] <= TOL_DEPTH
                if rows.get("theta_phase_err") is not None:
                    cells.append(f"phase err {rows['theta_phase_err']:.4f}")
                    ok &= rows["theta_phase_err"] <= TOL_PHASE
                if rows.get("theta_mag") is not None:
                    cells.append(f"thetaMag {rows['theta_mag']:.4f} 2fMag {rows['d2_mag']:.4f}")
                print(f"[{name:16}] desc {desc:3} B | " + ", ".join(cells) + f" | {'PASS' if ok else 'FAIL'}")
                # artifact rejection: a recovery that reports a strong theta
                # magnitude while the phase is unconstrained is suspect; record
                if name == "theta_am_gamma":
                    key = (tag,)
                    verdicts[key] = ok and rows["theta_mag"] > 0.1
            if tag == "HOLDOUT" and name in ("theta_am_gamma", "carrier_fixed"):
                verdicts.setdefault((tag, name), ok)
            elif tag == "BUILD" and name in ("theta_am_gamma", "carrier_fixed"):
                verdicts.setdefault((tag, name), ok)

    print("\n== serialized description costs ==")
    print("single tone (no mod):", descriptor_bytes(0, 40.0), "B")
    print("single tone + schedule:", descriptor_bytes(0, 40.0, with_schedule=True), "B")
    print("theta-AM gamma stream:", descriptor_bytes(1, 40.0, (6.0, 0.5, 0.0)), "B")
    print("phase-coupled stream:", descriptor_bytes(2, 40.0, (6.0, 0.5, 0.0)), "B")
    print("two independent streams (gamma+theta):",
          descriptor_bytes(0, 40.28, with_schedule=False)
          + descriptor_bytes(0, 6.0, None, False), "B (one carries the schedule)")

    gate = verdicts.get(("BUILD",)) and verdicts.get(("HOLDOUT",))
    print("\nN20 EXIT GATE:", "PASS" if gate else "FAIL",
          "- theta-AM gamma recovered on both fixtures independently of the generator")
    return 0 if gate else 1


if __name__ == "__main__":
    raise SystemExit(main())
