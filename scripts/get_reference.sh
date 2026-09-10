#!/bin/sh
# Fetch, verify and build the reference assembler: vasm 1.7h, vasmm68k_mot.
# Result: vasm-1.7h/vasmm68k_mot (the byte-exact oracle used by tests/).
# The vendored tree is not in git; run this once per checkout (macOS/Linux).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
URL=http://phoenix.owl.de/tags/vasm1_7h.tar.gz
SHA=5c012040cbfc2b16bdab0ad88dca31ec2f78b47b382c1048428ea1610063626d
cd "$ROOT"
if [ ! -f vasm1_7h.tar.gz ]; then
  echo "downloading $URL"
  curl -fsSL -o vasm1_7h.tar.gz "$URL"
fi
if command -v shasum >/dev/null 2>&1; then
  echo "$SHA  vasm1_7h.tar.gz" | shasum -a 256 -c -
else
  echo "$SHA  vasm1_7h.tar.gz" | sha256sum -c -
fi
rm -rf vasm-1.7h
mkdir vasm-1.7h
tar xzf vasm1_7h.tar.gz -C vasm-1.7h --strip-components=1
cp scripts/vasm-1.7h-Makefile.macOS vasm-1.7h/Makefile.macOS
echo "$SHA  vasm1_7h.tar.gz  (source: $URL)" > vasm-1.7h/SOURCE.sha256
make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot >/dev/null
echo "built: $ROOT/vasm-1.7h/vasmm68k_mot"
./vasm-1.7h/vasmm68k_mot -quiet -Fbin -o /dev/null /dev/null 2>/dev/null || true
./vasm-1.7h/vasmm68k_mot 2>&1 | head -1
