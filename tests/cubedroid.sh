#!/bin/sh
# Full-ROM acceptance gate: assemble CubeDroid with vasm_m and compare byte-exact
# against reference vasm 1.7h (raw assembler output, before the ROM padder).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REF="${REF:-$ROOT/vasm-1.7h/vasmm68k_mot}"
NEW="${NEW:-$ROOT/target/release/vasm_m}"
OUT="${OUT:-$ROOT/target/cubedroid}"
PROJ="$ROOT/AssemblyTest/CubeDroid"
[ -x "$REF" ] || { echo "reference vasm not built"; exit 2; }
[ -x "$NEW" ] || { echo "vasm_m not built: cargo build --release"; exit 2; }
rm -rf "$OUT" && mkdir -p "$OUT" || exit 2
cd "$PROJ" || exit 2
/usr/bin/time -p "$REF" -quiet -Fbin -spaces -o "$OUT/ref.bin" SourceCode/stub.X68 2>"$OUT/ref.log"; rs=$?
/usr/bin/time -p "$NEW" -quiet -Fbin -spaces -o "$OUT/new.bin" SourceCode/stub.X68 2>"$OUT/new.log"; ns=$?
echo "ref: exit=$rs $(grep real "$OUT/ref.log")   new: exit=$ns $(grep real "$OUT/new.log")"
[ $rs -eq 0 ] || { echo "FAIL: reference failed"; cat "$OUT/ref.log"; exit 1; }
[ $ns -eq 0 ] || { echo "FAIL: vasm_m failed"; grep -v real "$OUT/new.log" | head -20; exit 1; }
if cmp -s "$OUT/ref.bin" "$OUT/new.bin"; then
  echo "PASS: CubeDroid byte-exact ($(stat -f %z "$OUT/ref.bin") bytes)"
else
  echo "FAIL: CubeDroid differs: $(cmp -l "$OUT/ref.bin" "$OUT/new.bin" 2>/dev/null | wc -l | tr -d ' ') bytes; first: $(cmp "$OUT/ref.bin" "$OUT/new.bin" 2>&1 | head -1)"
  exit 1
fi
