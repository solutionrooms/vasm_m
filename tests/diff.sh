#!/bin/sh
# Differential acceptance test.
#
# Positive cases:  tests/corpus/NAME.s          both assemblers must exit 0 and
#                                               produce identical bytes.
# Negative cases:  tests/corpus/NAME.s + NAME.expect-fail
#                                               both must reject with exit 1
#                                               (a crash / signal is never a pass).
# Optional:        NAME.flags                   extra flags for both assemblers.
#
# Env: REF (reference vasm), NEW (vasm_m), FORMATS ("bin hunk"), OUT (out dir).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
# REF/NEW/OUT may be given relative to the launch directory; make them absolute
# before any cd (cubedroid.sh runs from the project directory).
abs() { case "$1" in /*) printf '%s\n' "$1";; *) printf '%s/%s\n' "$PWD" "$1";; esac; }
REF="${REF:-$ROOT/vasm-1.7h/vasmm68k_mot}"
NEW="${NEW:-$ROOT/target/release/vasm_m}"
FORMATS="${FORMATS:-bin}"
OUT="${OUT:-$ROOT/target/diff}"
REF=$(abs "$REF"); NEW=$(abs "$NEW"); OUT=$(abs "$OUT")
COMMON="-quiet -m68000"

[ -x "$REF" ] || { echo "reference vasm not built: make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot"; exit 2; }
[ -x "$NEW" ] || { echo "vasm_m not built: cargo build --release"; exit 2; }
rm -rf "$OUT" && mkdir -p "$OUT" || exit 2

# run with a 20 s timeout (exit 124 on timeout, which never matches an expected code)
run_to() { perl -e 'alarm 20; exec @ARGV' -- "$@"; }
set -- "$ROOT"/tests/corpus/*.s
[ -f "$1" ] || { echo "corpus is empty"; exit 2; }

pass=0; fail=0
for src in "$@"; do
  name=$(basename "$src" .s)
  flags=""; [ -f "${src%.s}.flags" ] && flags=$(cat "${src%.s}.flags")
  negative=0; [ -f "${src%.s}.expect-fail" ] && negative=1
  for fmt in $FORMATS; do
    ref="$OUT/$name.$fmt.ref"; new="$OUT/$name.$fmt.out"
    run_to "$REF" $COMMON -F$fmt $flags -o "$ref" "$src" >"$ref.log" 2>&1; rs=$?
    run_to "$NEW" $COMMON -F$fmt $flags -o "$new" "$src" >"$new.log" 2>&1; ns=$?
    if [ $negative -eq 1 ]; then
      if [ $rs -ne 1 ]; then
        echo "FAIL $name [$fmt]: negative case but reference exited $rs (expected 1)"; fail=$((fail+1))
      elif [ $ns -ne 1 ]; then
        echo "FAIL $name [$fmt]: vasm_m exited $ns, expected rejection (1)"; fail=$((fail+1))
      else
        pass=$((pass+1))
      fi
    else
      if [ $rs -ne 0 ]; then
        echo "FAIL $name [$fmt]: reference exited $rs on a positive case"; sed 's/^/    ref: /' "$ref.log" | head -5; fail=$((fail+1))
      elif [ $ns -ne 0 ]; then
        echo "FAIL $name [$fmt]: vasm_m exited $ns"; sed 's/^/    new: /' "$new.log" | head -5; fail=$((fail+1))
      elif [ ! -f "$new" ]; then
        echo "FAIL $name [$fmt]: vasm_m produced no output file"; fail=$((fail+1))
      elif ! cmp -s "$ref" "$new"; then
        echo "FAIL $name [$fmt]: bytes differ ($(stat -f %z "$ref") vs $(stat -f %z "$new"))"; cmp "$ref" "$new" 2>&1 | head -1 | sed 's/^/    /'; fail=$((fail+1))
      else
        pass=$((pass+1))
      fi
    fi
  done
done
echo "pass=$pass fail=$fail"
[ $fail -eq 0 ]
