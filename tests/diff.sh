#!/bin/sh
# Differential test: assemble every tests/corpus/*.s with reference vasm and vasm_m,
# compare output bytes for each format. Exit non-zero on any mismatch.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
REF="${REF:-$ROOT/vasm-1.7h/vasmm68k_mot}"
NEW="$ROOT/target/release/vasm_m"
FORMATS="${FORMATS:-bin hunk}"
OUT="${OUT:-$ROOT/target/diff}"
[ -x "$REF" ] || { echo "reference vasm not built: make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot"; exit 2; }
[ -x "$NEW" ] || { echo "vasm_m not built: cargo build --release"; exit 2; }
mkdir -p "$OUT"
pass=0; fail=0
for src in "$ROOT"/tests/corpus/*.s; do
  name=$(basename "$src" .s)
  flags=""; [ -f "${src%.s}.flags" ] && flags=$(cat "${src%.s}.flags")
  for fmt in $FORMATS; do
    ref="$OUT/$name.$fmt.ref"; new="$OUT/$name.$fmt.out"
    "$REF" -quiet -F$fmt $flags -o "$ref" "$src" >"$ref.log" 2>&1; rs=$?
    "$NEW" -quiet -F$fmt $flags -o "$new" "$src" >"$new.log" 2>&1; ns=$?
    if [ $rs -ne $ns ]; then
      echo "FAIL $name [$fmt]: exit ref=$rs new=$ns"; fail=$((fail+1))
    elif [ $rs -eq 0 ] && ! cmp -s "$ref" "$new"; then
      echo "FAIL $name [$fmt]: bytes differ"; cmp "$ref" "$new" | head -1; fail=$((fail+1))
    else
      pass=$((pass+1))
    fi
  done
done
echo "pass=$pass fail=$fail"
[ $fail -eq 0 ]
