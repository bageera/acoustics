#!/usr/bin/env python3
"""G23-H holdout fixture: derive the deterministic 16-bit stereo holdout from
the untracked 24-bit g12 corpus file park_wind.wav.

Output: testdata/holdouts/park_wind16.wav (1,536,044 bytes, committed).
Derivation: FIRST 8.0 seconds of the 48 kHz stereo source, 24-to-16-bit by
taking each little-endian 24-bit sample's top two bytes (an exact >>8).

Re-run after re-fetching the g12 corpus:
    python3 scripts/g23_holdout.py
"""

import os
import struct
import sys

SRC = "testdata/g12/park_wind.wav"
OUT = "testdata/holdouts/park_wind16.wav"

def read_wav(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[0:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise SystemExit("not a RIFF/WAVE file")
    i = 12
    fmt = None
    payload = None
    while i < len(data) - 8:
        cid = data[i : i + 4]
        (size,) = struct.unpack("<I", data[i + 4 : i + 8])
        if cid == b"fmt ":
            fmt = data[i + 8 : i + 8 + 16]
        if cid == b"data":
            payload = data[i + 8 : i + 8 + size]
            break
        i += 8 + size + (size & 1)
    if fmt is None or payload is None:
        raise SystemExit("fmt/data chunk missing")
    _, chans, rate, _byte_rate, _, bits = struct.unpack("<HHIIHH", fmt)
    return chans, rate, bits, payload

def main():
    chans, rate, bits, payload = read_wav(SRC)
    if bits != 24 or chans != 2 or rate != 48000:
        raise SystemExit(f"unexpected source format: {bits}-bit {chans}ch {rate} Hz")
    bytes_per_frame = chans * 3
    frames_total = len(payload) // bytes_per_frame
    frames = min(int(rate * 8.0), frames_total)
    os.makedirs("testdata/holdouts", exist_ok=True)
    samples = bytearray()
    for idx in range(frames * chans):
        b = payload[idx * 3 : idx * 3 + 3]
        samples += bytes([b[2], b[1]])  # the >>8 top two bytes
    import wave

    out = wave.open(OUT, "wb")
    out.setnchannels(chans)
    out.setsampwidth(2)
    out.setframerate(rate)
    out.writeframes(bytes(samples))
    out.close()
    print(f"wrote {OUT}: {os.path.getsize(OUT)} bytes ({frames} frames @ {rate} Hz)")

if __name__ == "__main__":
    sys.exit(main())
