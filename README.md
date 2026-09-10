# vasm_m

The intended project is a modern, multithreaded 68000 assembler compatible with
vasm. The current milestone is a reproducible local reference build; the new
assembler has not been implemented yet.

## Agreed scope

- Implementation language: Rust.
- Target CPU: original Motorola 68000 only.
- Syntax: `vasmm68k_mot` compatibility only.
- Phase 1 output formats: raw binary and Amiga Hunk. ELF follows in phase 2.
- Acceptance target: byte-for-byte identical output to the pinned reference
  vasm with the same source, options, and relevant input paths. This includes
  matching optimization and branch-relaxation results for supported options.
  Any exception for object metadata must be explicitly agreed, not silently
  normalized away by the comparison tools.
- CLI: prefer familiar vasm flags and drop-in usage where practical; complete
  CLI compatibility is secondary to output correctness.
- Listings, symbol dumps, and listing-file formatting need not match. This
  does not relax correctness requirements for symbols inside object files.
- Output must remain identical across thread counts.

Byte equality makes comparison unambiguous, but is a stricter implementation
target than functional equivalence. Matching vasm's results does not necessarily
require using its internal algorithms. Establish a deterministic single-threaded
implementation and differential tests before enabling parallel execution.

## Reference vasm on macOS

The compatibility reference is `vasm-1.7h/`: it reproduces the supplied CubeDroid
ROM exactly after the game's padding and header patches. Its source archive
checksum and URL are recorded in `vasm-1.7h/SOURCE.sha256`. The newer `vasm/`
tree (2.0f) remains available for secondary checks, but is not the byte-exact
target. Both macOS makefiles build with Apple Clang at `-O2`. Xcode or the Xcode
Command Line Tools provide the compiler and make.

Run from this directory:

```sh
make -C vasm-1.7h -B -f Makefile.macOS CPU=m68k SYNTAX=mot -j4
./vasm-1.7h/vasmm68k_mot
python3 scripts/check_reference.py
```

`-B` rebuilds every object, preventing reuse of objects produced with different
compiler flags. The executables are `vasm-1.7h/vasmm68k_mot` and `vasm-1.7h/vobjdump`.
No system-wide installation is needed. The makefile suppresses several classes
of legacy-source warnings; a quiet build does not mean every warning is enabled.

Example assembly for the original 68000, producing a raw binary:

```sh
./vasm-1.7h/vasmm68k_mot -m68000 -Fbin -o /tmp/encoding.bin tests/reference/encoding.s
```

Always specify the output format: vasm's default is its diagnostic test format.
Select the eventual project's format and compatibility/optimization flags
explicitly. The included smoke check verifies 16 independently specified bytes,
repeatability, and rejection of an instruction requiring a later CPU. It does
not establish complete instruction or output-format correctness.
Set `REF` to another executable to run the same smoke check against it, for
example `REF=./vasm/vasmm68k_mot python3 scripts/check_reference.py`.

Verified locally on 2026-09-10: native macOS arm64, Apple Clang 21.0.0, GNU Make
3.81. The full rebuild completed successfully with no emitted warnings using
the supplied macOS makefile.

Independent CubeDroid verification with 1.7h produced 944,216 raw bytes (SHA-256
`23a13f9973b19bb50b34657c07d966beb707ee3ef068575a8550a9f713d3a549`).
Three runs produced identical output; explicitly adding `-m68000` also matched.
The shipped ROM is 1,048,576 bytes: the raw output differs at five header bytes
and lacks the final zero padding. Reproducing the checked-in padder's header
patches and padding gives the exact shipped SHA-256
`40537c7bf45344da35ee758c143a4737eb1813830d7938768b74c755e75ae1e9`.
Assembler differential tests should compare raw outputs directly, before padding.

## Compatibility and performance direction

- Use the supplied reference version as the initial comparison target. Record
  its source checksum, exact flags, inputs, and toolchain with benchmark results
  before changing or replacing the reference source.
- Aim for byte-for-byte equality of emitted code and data with identical
  options. For object formats, also compare sections, symbols, and relocations;
  document any permitted differences in metadata.
- Cover syntax, macros, includes, expressions, local labels, conditional
  assembly, alignment, addressing modes, and branch relaxation. Similar output
  size alone is insufficient evidence of correctness.
- Require deterministic output at every supported thread count. Start with
  independent translation units where the real project provides them. Profile
  before deciding how to parallelize work within one translation unit: include
  files can share parser state, and branch sizes depend on final layout.
- Compare wall time, CPU time, peak memory, and emitted size on the same inputs
  and options, using repeated runs and recording thread count. Separate assembler
  timings from linking and other build steps.

The example project should determine the first concrete compatibility flags and
whether its workload is one large translation unit or many independent files.
The threading architecture remains open until that workload has been inspected
and the reference build profiled.
