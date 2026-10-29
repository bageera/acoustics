#!/usr/bin/env bash
# G12 corpus fetcher — acoustic-diversity expansion (CC0, hash-locked).
# Files are LARGE (262 MB total): stored under testdata/g12/ which is
# GITIGNORED — acquisition is reproducible from this script + the locked
# hashes (the repo carries the recipe, not the bytes; G5-F's small corpus
# stays in-repo for its invariants).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p testdata/g12
export UA="acoustics-g12/0.1 (github.com/bageera/acoustics)"

fetch() {
  local name="$1" url="$2" want="$3"
  local out="testdata/g12/$name"
  if [ -f "$out" ] && [ "$(shasum -a 256 "$out" | awk '{print $1}')" = "$want" ]; then
    echo "have (verified): $out"
    return
  fi
  curl -sL -m 600 -A "$UA" "$url" -o "$out"
  local got
  got="$(shasum -a 256 "$out" | awk '{print $1}')"
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name hash mismatch (got $got, want $want)" >&2
    exit 1
  fi
  echo "fetched (verified): $out"
}

# Wikimedia (CC0): piano performance, 16-bit stereo 44.1k
fetch emotional_piano.wav \
  "https://commons.wikimedia.org/wiki/Special:FilePath/Emotional_piano.wav" \
  "9661f81d37c59f230b324b830ab68c0482336af3ec117c92e73108ffb4095f15"
# OpenGameArt (CC0): 16-bit stereo 48k field recordings
fetch park_birds.wav \
  "https://opengameart.org/sites/default/files/park_ambience_birds.wav" \
  "dc1a5ac34b89cab7f8f261b53a23da7c1c5e3cc895aae4cb6fb4a1a04001b2af"
fetch park_river.wav \
  "https://opengameart.org/sites/default/files/park_ambience_river.wav" \
  "da0c0c9b1249a83a1611cc116d5737b0864a8db9f45f1ae8dcc079d0f7c0bbbf"
# OpenGameArt (CC0): 24-BIT — UNSUPPORTED by the engine (i16-only): kept in
# original form, NEVER converted (the review's preservation rule); recorded
# as a source whose measurement waits for 24-bit support.
fetch park_wind.wav \
  "https://opengameart.org/sites/default/files/park_ambience_wind.wav" \
  "5c381856745b4706e7eba55eb9271a61a530e90c05df8126adb2db89ecfa6c5a"

file testdata/g12/*.wav
echo "NOT ACQUIRED (blocked):"
echo "  - freesound 508800 piano single note (24-bit; freesound download requires an API token - CC0, skipped honestly)"
echo "  - park_wind.wav acquired above but 24-bit: engine measurement deferred (supported only after true 24-bit support)"
