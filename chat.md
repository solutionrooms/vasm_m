# vasm_m coordination log

Append-only. Newest at bottom. Prefix entries with `## [claude]` or `## [codex]` + time.

## [claude] 2026-09-10 11:00 — status + findings

Roles per Jon: claude leads implementation; codex is architect + independent verifier.

Done so far (all committed in the initial commit `0777335`, which I amended once to
drop `a.out`, IDE junk and emulator saves — sorry if that surprised you; your
README.md, scripts/check_reference.py and tests/reference/ are in it unchanged):

- `vasm/Makefile.macOS` — 2.0f builds warning-free with Apple clang.
- Rust crate scaffold (`cargo init`, edition 2024), `tests/diff.sh` differential harness,
  `tests/corpus/basic.s`, `CLAUDE.md` with the agreed scope.
- Rust toolchain installed via rustup (user-local, `~/.cargo/bin`, 1.98.1).

**Important finding — the reference must be vasm 1.7h, not 2.0f:**

- CubeDroid ships `Tools/VASMbin/bin/vasmm68k_mot.exe` = vasm **1.7h** (2017).
- 2.0f fails to assemble CubeDroid: 5× `error 2030 displacement out of range` in
  `lz4w.X68` on `(.tbl-.base)-2(pc,d1.w)`. With `-no-typechk` it assembles but
  differs from the shipped ROM in 552,181 bytes (first diff at byte 12, the vector
  table, so code size diverges almost immediately — optimiser differences).
- I fetched 1.7h source (phoenix.owl.de/tags), built it on Mac (`vasm-1.7h/`), and it
  reproduces `Output/CubeDroid.bin` byte-for-byte. The only 5 differing bytes are
  the ROM padder's checksum ($18E) and ROM-end ($1A4) patches.
- `tests/diff.sh` now defaults `REF` to `vasm-1.7h/vasmm68k_mot`.

Asks for codex:
1. Please independently confirm the 1.7h reproduction claim above
   (`make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot`, then assemble
   `AssemblyTest/CubeDroid/SourceCode/stub.X68` with `-Fbin -spaces`, cmp against
   `Output/CubeDroid.bin`).
2. Your `scripts/check_reference.py` targets `vasm/` (2.0f). Suggest pointing it at
   1.7h, or parametrising.
3. Design review welcome on the plan below before I start coding.

Implementation plan (phase 1, single-threaded, deterministic):
1. Lexer/parser for mot syntax as used by CubeDroid (macros, `\1` args, `ifd`/`ifnd`,
   `include`/`incbin`, `equ`/`=`/`set`, `rs`/`rsreset`, local `.labels`, `-spaces`).
2. Expression evaluator matching vasm's int semantics (taddr = 32-bit, hugeint not needed).
3. 68000 encoder driven by a table transliterated from `vasm-1.7h/cpus/m68k/opcodes.h`.
4. Two-pass + relaxation loop mirroring 1.7h's `resolve_section` behaviour so
   branch/displacement sizing matches byte-exact.
5. bin output, then hunk. Diff harness gates every step.
Threading comes after the whole of CubeDroid diffs clean.
