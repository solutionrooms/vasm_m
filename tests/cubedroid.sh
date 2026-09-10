#!/bin/sh
# Full-ROM acceptance gate: assemble CubeDroid with vasm_m and compare byte-exact
# against reference vasm 1.7h (raw assembler output, before the ROM padder).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
# REF/NEW/OUT may be given relative to the launch directory; make them absolute
# before any cd (cubedroid.sh runs from the project directory).
abs() { case "$1" in /*) printf '%s\n' "$1";; *) printf '%s/%s\n' "$PWD" "$1";; esac; }
REF="${REF:-$ROOT/vasm-1.7h/vasmm68k_mot}"
NEW="${NEW:-$ROOT/target/release/vasm_m}"
OUT="${OUT:-$ROOT/target/cubedroid}"
REF=$(abs "$REF"); NEW=$(abs "$NEW"); OUT=$(abs "$OUT")
PROJ="${PROJ:-$ROOT/AssemblyTest/CubeDroid}"
[ -x "$REF" ] || { echo "reference vasm not found: $REF (run scripts/get_reference.sh or set REF)"; exit 2; }
[ -f "$PROJ/SourceCode/stub.X68" ] || { echo "CubeDroid project not found: $PROJ (not in git; copy AssemblyTest/ into the repo root or set PROJ)"; exit 2; }
[ -x "$NEW" ] || { echo "vasm_m not built: cargo build --release"; exit 2; }
rm -rf "$OUT" && mkdir -p "$OUT" || exit 2
cd "$PROJ" || exit 2
status=0
for fmt in bin hunk hunkexe; do
  /usr/bin/time -p "$REF" -quiet -F$fmt -spaces -o "$OUT/ref.$fmt" SourceCode/stub.X68 2>"$OUT/ref.$fmt.log"; rs=$?
  /usr/bin/time -p "$NEW" -quiet -F$fmt -spaces -o "$OUT/new.$fmt" SourceCode/stub.X68 2>"$OUT/new.$fmt.log"; ns=$?
  echo "$fmt: ref exit=$rs $(grep real "$OUT/ref.$fmt.log")   new exit=$ns $(grep real "$OUT/new.$fmt.log")"
  if [ $rs -ne 0 ]; then echo "FAIL [$fmt]: reference failed"; cat "$OUT/ref.$fmt.log"; status=1
  elif [ $ns -ne 0 ]; then echo "FAIL [$fmt]: vasm_m failed"; grep -v real "$OUT/new.$fmt.log" | head -20; status=1
  elif cmp -s "$OUT/ref.$fmt" "$OUT/new.$fmt"; then
    echo "PASS: CubeDroid byte-exact [$fmt] ($(stat -f %z "$OUT/ref.$fmt") bytes)"
  else
    echo "FAIL [$fmt]: CubeDroid differs: $(cmp -l "$OUT/ref.$fmt" "$OUT/new.$fmt" 2>/dev/null | wc -l | tr -d ' ') bytes; first: $(cmp "$OUT/ref.$fmt" "$OUT/new.$fmt" 2>&1 | head -1)"; status=1
  fi
done
exit $status
