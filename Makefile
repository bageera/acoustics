# ACOUSTICS — Makefile: development dependency + workflow entry points.
#
# The crate itself ships with ZERO Cargo dependencies (Cargo.toml). What this
# Makefile manages are the DEVELOPMENT prerequisites:
#   1. Rust toolchain — cargo + rustc + the rustfmt/clippy components
#      (rustup: https://rustup.rs)
#   2. python3 — needed only by `roundtrip` and scripts/release-gate.sh
#      (the stage-1 WAV fixture)
#   3. Test corpora — testdata/g5f (small, in-repo, re-fetchable via
#      scripts/g5f_fetch.sh) and testdata/g12 (~262 MB, gitignored,
#      hash-locked via scripts/g12_fetch.sh) behind the 11 corpus-gated tests
#
# Default target `all` mirrors release-gate stage 1: fmt --check +
# clippy -D warnings + cargo test --release --locked.
#
# Compat: written for GNU Make 3.81 (the macOS default) — no 3.82+ features.

SHELL := /bin/bash

.PHONY: all help deps check-deps corpus g5f g12 build release test test-release \
        test-corpus fmt fmt-check clippy roundtrip audit clean gate publish

all: fmt-check clippy test-release

help:
	@echo "ACOUSTICS make targets:"
	@echo "  all             fmt-check + clippy + release tests (gate stage 1)"
	@echo "  deps            toolchain check + the small in-repo g5f corpus"
	@echo "  check-deps      verify cargo/rustfmt/clippy/python3 presence"
	@echo "  corpus          fetch all corpora (g5f + g12; first run ~262 MB)"
	@echo "  g5f / g12       fetch one corpus script directly"
	@echo "  test            cargo test (debug)"
	@echo "  test-release    cargo test --release --locked (gate stage 1)"
	@echo "  test-corpus     the corpus-gated tests (fetches corpora first)"
	@echo "  fmt / fmt-check cargo fmt --all [-- --check]"
	@echo "  clippy          cargo clippy --all-targets -- -D warnings"
	@echo "  roundtrip       CLI encode/decode must be PCM byte-identical"
	@echo "  audit           cargo audit (advisory DB only; zero-dep crate)"
	@echo "  build / release cargo build [--release] --locked"
	@echo "  clean           cargo clean"
	@echo "  gate V=x.y.z    scripts/release-gate.sh (V must equal Cargo.toml)"
	@echo "  publish V=x.y.z gate + crates.io publish (sets ACOUSTICS_RELEASE=1)"

# ---------- dependencies ----------

check-deps:
	@command -v cargo >/dev/null 2>&1 || { \
	  echo "FATAL: cargo not on PATH — install rustup (https://rustup.rs):"; \
	  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"; \
	  echo "  then: source \"$$HOME/.cargo/env\""; \
	  exit 1; \
	}
	@cargo --version
	@cargo fmt --version >/dev/null 2>&1 || \
	  echo "WARN: rustfmt missing — run: rustup component add rustfmt"
	@cargo clippy --version >/dev/null 2>&1 || \
	  echo "WARN: clippy missing — run: rustup component add clippy"
	@command -v python3 >/dev/null 2>&1 || \
	  echo "WARN: python3 not found — needed only by 'roundtrip' and 'gate'"

g5f:
	bash scripts/g5f_fetch.sh

g12:
	bash scripts/g12_fetch.sh

corpus: g5f g12

deps: check-deps g5f

# ---------- builds and tests ----------

build:
	cargo build

release:
	cargo build --release --locked

test:
	cargo test

test-release:
	cargo test --release --locked

test-corpus: corpus
	cargo test --release --locked -- --ignored

# ---------- checks ----------

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets -- -D warnings

# stage-1's exact-PCM check, standalone: generated two-tone fixture encodes,
# decodes through the release CLI, and must come back byte-identical.
roundtrip:
	@set -euo pipefail; \
	  command -v python3 >/dev/null 2>&1 || { \
	    echo "FATAL: python3 required for the roundtrip fixture"; exit 1; \
	  }; \
	  T="$$(mktemp -d)"; \
	  trap 'rm -rf "$$T"' EXIT; \
	  python3 -c 'import sys,math,wave;out=sys.argv[1];w=wave.open(out,"wb");w.setnchannels(1);w.setsampwidth(2);w.setframerate(44100);w.writeframes(b"".join(int(max(-32768,min(32767,round(8000.0*math.cos(2*math.pi*917.3*i/44100)+5000.0*math.cos(2*math.pi*1361.0*i/44100))))).to_bytes(2,"little",signed=True) for i in range(4096)));w.close()' "$$T/fixture.wav"; \
	  cargo run --release --locked --bin acoustics -- benchmark "$$T/fixture.wav" "$$T/out.acoust" >/dev/null; \
	  cargo run --release --locked --bin acoustics -- decode "$$T/out.acoust" "$$T/back.wav" >/dev/null; \
	  cmp "$$T/fixture.wav" "$$T/back.wav"; \
	  echo "roundtrip: PCM byte-identical"

audit:
	@command -v cargo-audit >/dev/null 2>&1 || { \
	  echo "FATAL: cargo-audit not installed — run: cargo install cargo-audit"; \
	  exit 1; \
	}
	cargo audit

clean:
	cargo clean

# ---------- release (delegate to the existing gate script) ----------

gate:
	@test -n "$(V)" || { echo "usage: make gate V=<version> (must equal the Cargo.toml version)"; exit 1; }
	./scripts/release-gate.sh $(V)

publish:
	@test -n "$(V)" || { echo "usage: make publish V=<version> — immutable step, human decision"; exit 1; }
	ACOUSTICS_RELEASE=1 ./scripts/release-gate.sh $(V)
