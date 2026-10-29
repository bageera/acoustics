#!/usr/bin/env bash
# G5-F corpus fetcher — the locked real-audio evaluation set (CC0, Wikimedia).
# Re-running is idempotent: fetch → verify sha256 → convert nothing (no resampling;
# stereo stays stereo; the harness reads channels separately).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p testdata/g5f
export UA="acoustics-g5f/0.1 (github.com/bageera/acoustics)"

fetch() {
  local name="$1" url="$2" want="$3"
  local out="testdata/g5f/$name"
  if [ -f "$out" ] && [ "$(shasum -a 256 "$out" | awk '{print $1}')" = "$want" ]; then
    echo "have (verified): $out"
    return
  fi
  curl -sL -m 180 -A "$UA" "$url" -o "$out"
  local got
  got="$(shasum -a 256 "$out" | awk '{print $1}')"
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name hash mismatch (got $got, want $want)" >&2
    exit 1
  fi
  echo "fetched (verified): $out"
}

fetch example_word.wav "https://commons.wikimedia.org/wiki/Special:FilePath/The_English_word_EXAMPLE.wav" "b8fe12af732956f31aab51969ddf6b3178b41ab3505745a8ac2f851e754bea5b"
fetch can_you_stop.wav "https://commons.wikimedia.org/wiki/Special:FilePath/Can_you_stop_here_for_a_minute.wav" "ea1aec121f06ae5eccd6469f6cf0e950ed106fd97048971986f5807bbf5ad3ca"
fetch sample2.wav "https://commons.wikimedia.org/wiki/Special:FilePath/Sample2.wav" "e2b03e05f08577af9ef8051cdb723e892f92e554a1b34b276898f57650f28ed8"

file testdata/g5f/*.wav