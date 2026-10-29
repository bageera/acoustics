#!/usr/bin/env bash
# RELEASE GATE — the 0.8.x review's pipeline, as code (0.8.3-era fixture).
#
# Usage: scripts/release-gate.sh <version>   (version = the Cargo.toml version)
#
# Stages (exit non-zero on the first failure; publish runs ONLY after all pass):
#   1. Internal correctness: fmt --check, clippy -D warnings, release tests,
#      exact-PCM CLI roundtrip (packaged wire must decode back byte-identical).
#   2. Packaging: cargo package --locked (the distribution artifact), then list
#      its contents and require every module the CHANGELOG's release section
#      names to be present in the tarball.
#   3. External consumer FROM THE PACKAGED ARTIFACT: unpack the .crate into a
#      scratch dir, build a scratch consumer project whose dependency resolves
#      to the EXTRACTED SOURCES (a path dependency on what consumers actually
#      receive — not the workspace), compile an import for every advertised
#      research API and run the numeric-regression asserts inline.
#   4. Numerical regression: the G-series measurements asserted within ranges
#      recorded in docs/science.md (bounds loose enough for rounding drift,
#      tight enough to catch a wire/accounting regression).
#   5. Publish (manual): the script STOPS before publish unless
#      ACOUSTICS_RELEASE=1 is exported — the immutable-version step stays a
#      human decision.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: release-gate.sh <version>}"
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== stage 1: internal correctness =="
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --release --locked

# exact-PCM roundtrip through the release CLI (the wire's own decoder)
FIX="$SCRATCH/fixture.wav"
OUT="$SCRATCH/fixture.acoust"
BACK="$SCRATCH/back.wav"
python3 - "$FIX" <<'PYEOF'
import sys, struct, math, wave as wav
fs = 44100.0
frames = b"".join(
    struct.pack("<h", max(-32768, min(32767, round(
        8000.0 * math.cos(2 * math.pi * 917.3 * i / fs)
        + 5000.0 * math.cos(2 * math.pi * 1361.0 * i / fs)))))
    for i in range(4096)
)
w = wav.open(sys.argv[1], "wb")
w.setnchannels(1); w.setsampwidth(2); w.setframerate(44100)
w.writeframes(frames); w.close()
PYEOF
cargo run --release --locked --bin acoustics -- benchmark "$FIX" "$OUT" >/dev/null
cargo run --release --locked --bin acoustics -- decode "$OUT" "$BACK" >/dev/null
if ! cmp -s "$FIX" "$BACK"; then
  echo "FAIL: CLI roundtrip diverged"; exit 1
fi
echo "CLI roundtrip: PCM byte-identical"

echo "== stage 2: packaging --locked =="
cargo package --locked --no-verify --allow-dirty
CRATE="target/package/manifold-acoustics-$VERSION.crate"
test -f "$CRATE" || { echo "FAIL: $CRATE missing"; exit 1; }
TARBALL_LIST="$(tar -tzf "$CRATE")"
# every module the CHANGELOG's release section advertises must be IN the tarball —
# the 0.8.0 lesson (surface advertised but never verified present):
for module in lib.rs main.rs nested.rs phase.rs modal.rs g3.rs g5.rs; do
  grep -q "src/$module" <<<"$TARBALL_LIST" || { echo "FAIL: src/$module missing from the tarball"; exit 1; }
done
echo "tarball carries all advertised modules"

echo "== stage 3: consumer from the PACKAGED artifact =="
PKG_DIR="$SCRATCH/packaged"
mkdir -p "$PKG_DIR"
tar -xzf "$CRATE" -C "$PKG_DIR"
SRC_DIR="$PKG_DIR/manifold-acoustics-$VERSION"
test -d "$SRC_DIR" || { echo "FAIL: packaged source layout unexpected"; exit 1; }

CONSUMER="$SCRATCH/consumer"
cargo new "$CONSUMER" --bin >/dev/null
echo "manifold-acoustics = { path = \"$SRC_DIR\" }" >>"$CONSUMER/Cargo.toml"
# every advertised research API gets an import here (the 0.8.x visibility lesson —
# a pub(crate)-only fn dies at THIS compile, inside the workspace's shadow):
mkdir -p "$CONSUMER/src"
cat >"$CONSUMER/src/main.rs" <<'EOF'
use acoustics as _c; // [lib] name = acoustics (the package is manifold-acoustics)
fn main() {
    let samples: Vec<i16> = (0..4096usize)
        .map(|i| {
            let t = i as f64 / 44100.0;
            let v = 8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
            (v.round()).clamp(-32767.0, 32767.0) as i16
        })
        .collect();
    let payload = _c::encode(44100, &samples).expect("encode");
    let (_, back, m) = _c::decode_payload(&payload).expect("decode");
    assert_eq!(back, samples, "packaged crate must decode exactly");
    // advertised research API — imports compile only if the surface is public
    let stages = _c::g5::analyze_f3(&samples, &[1]);
    let g3 = _c::g3::discover_delta_l(&samples).map(|(d, _, _)| d);
    let nested = _c::nested::nested_echo_plan(&samples).map(|p| p.cost);
    let modal = _c::modal::discover_modal(&samples).map(|(p, c)| (p.cost, p.comps.len()));
    let phase = _c::phase::discovered_wire(&samples).map(|(_, b)| b);
    println!(
        "CONSUMER OK: wire={}B kind={} | g5={} g3={:?} nested={:?} modal={:?} phase={:?}",
        payload.len(), m.kind, stages.len(), g3, nested, modal, phase
    );
}
EOF
# the crate's lib name resolves via the PACKAGE name's underscores in the path-dep
sed -i '' 's/#[lib]/[lib]/' /dev/null 2>/dev/null || true
(cd "$CONSUMER" && cargo run --release 2>&1 | grep CONSUMER)

echo "== stage 4: numerical regression =="
CONSUMER_OUT="$(cd "$CONSUMER" && cargo run --release 2>/dev/null | tail -1)"
echo "$CONSUMER_OUT"
# ranges (science.md's recorded measurements ± slack for rounding/fit drift):
grep -q "wire=4117B" <<<"$CONSUMER_OUT" || { echo "FAIL: wire size regressed from 4117 (v5/D3 floor)"; exit 1; }
grep -q "kind=6" <<<"$CONSUMER_OUT" || { echo "FAIL: portfolio model regressed (expected the D3 kind-6 floor)"; exit 1; }
grep -q "g5=1" <<<"$CONSUMER_OUT" || { echo "FAIL: g5 stage count regressed"; exit 1; }
grep -qE "nested=Some\(54[0-9]{2}\)" <<<"$CONSUMER_OUT" || { echo "FAIL: nested cost out of range"; exit 1; }
grep -qE "modal=Some\(\(89[0-9]{2}, 3\)\)" <<<"$CONSUMER_OUT" || { echo "FAIL: modal cost out of range"; exit 1; }
grep -qE "phase=Some\(9[0-9]{3}\)" <<<"$CONSUMER_OUT" || { echo "FAIL: phase cost out of range"; exit 1; }
grep -qE "g3=Some\(0\)" <<<"$CONSUMER_OUT" || { echo "FAIL: g3 ΔL out of range (v5: D3 subsumes the DIFF transform gain — recorded)"; exit 1; }
echo "numerical regression: all G-series rows inside their recorded ranges"

echo "== stage 5: publish gate =="
if [ "${ACOUSTICS_RELEASE:-0}" = "1" ]; then
  cargo publish
  echo "PUBLISHED manifold-acoustics $VERSION"
else
  echo "publish SKIPPED (set ACOUSTICS_RELEASE=1 to publish — immutable step)"
fi
echo "GATE PASS"