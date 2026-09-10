# vasm_m — multithreaded 68000 assembler, byte-compatible with vasm

## Goal
A modern, multithreaded reimplementation of `vasmm68k_mot` in Rust. Output must be
**byte-exact** with the reference vasm build for the same input and flags.

## Decisions (2026-09-10)
- Language: Rust (edition 2024).
- Scope: `vasmm68k_mot` only (Motorola syntax module). CPU: plain 68000 only
  (no 68010+, no ColdFire/CPU32/FPU/MMU).
- Output formats: hunk + bin first; ELF is phase 2.
- Listing/symbol-dump output: not required to match.
- CLI: ideally drop-in for vasmm68k_mot flags; not a hard requirement.
- Comparison standard: byte-exact object output vs reference vasm.

## Roles
- **claude** leads implementation: owns code changes, delivers milestones.
- **codex** is architect: reviews designs, independently verifies output vs vasm,
  checks performance claims, investigates discrepancies.
- Coordinate via `chat.md` at repo root (append-only, newest at bottom, prefix
  entries with `## [claude]` / `## [codex]` and a timestamp).

## Reference vasm — 1.7h is the byte-exact target
- `vasm-1.7h/` is vasm **1.7h** (m68k backend 2.1c, mot syntax 3.9e), the version the
  example project ships (`AssemblyTest/Tools/VASMbin/bin/vasmm68k_mot.exe`). Source
  from http://phoenix.owl.de/tags/vasm1_7h.tar.gz, checksum in `vasm-1.7h/SOURCE.sha256`.
  Mac build reproduces `AssemblyTest/CubeDroid/Output/CubeDroid.bin` exactly
  (only the 5 bytes the ROM padder patches differ).
- `vasm/` is vasm 2.0f, kept for docs and for phase-2 comparison. It does NOT build
  CubeDroid (5 "displacement out of range" errors in lz4w.X68) and its output differs
  from 1.7h in ~550k bytes, so it is not the compatibility target.
- Build either: `make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot`
  (same for `vasm`). Do not modify vendored sources except build files.

## Example project: AssemblyTest/CubeDroid (Mega Drive game)
- Build command: `vasmm68k_mot -o Output/CubeDroid.bin -Fbin -spaces SourceCode/stub.X68`
  run from `AssemblyTest/CubeDroid/`. Single translation unit, ~60 includes, 944216 bytes out.
- Reference 1.7h assembles it in ~0.6 s on M-series. That is the number to beat.
- `SnakeRomPadder` post-processes: pads to power-of-two, patches $1A4 (ROM end) and
  $18E (checksum). Not the assembler's job.

## Layout
- `src/` — the Rust assembler.
- `tests/corpus/` — `.s` sources used for differential testing.
- `tests/diff.sh` — assembles each corpus file with both assemblers and diffs the bytes.
- `examples/` — the real-world example project (to be added).

## Workflow
- `cargo build --release && tests/diff.sh` is the acceptance test. Any diff is a bug.
- Add a corpus file for every encoder/directive feature as it's implemented.
