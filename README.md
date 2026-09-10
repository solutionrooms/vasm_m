# vasm_m

`vasm_m` is a Rust implementation of a Motorola-syntax 68000 assembler, targeting
byte-exact compatibility with vasm 1.7h. It currently runs single-threaded and
supports raw binary (`-Fbin`), Amiga Hunk objects (`-Fhunk`) and Hunk executables
(`-Fhunkexe`). Parallel assembly and ELF output are not implemented yet.

Independent review of commit `19bc3da` confirmed exact CubeDroid output in all
three formats and 66 passing corpus checks. CubeDroid assembled in about 123–124
ms in those individual runs, versus 607–650 ms for the reference on this M1 Max.
These timings were taken during concurrent development, not a controlled benchmark.
Broader compatibility testing is ongoing: the review found a `-databss` trimming
mismatch outside CubeDroid (constant-data merging changed the trimming
granularity); fixed in the following commit, reproducer kept as
`tests/corpus/codex_hunk_databss_merge.s`.

## Try it

### Get the source

```sh
git clone https://github.com/solutionrooms/vasm_m.git
cd vasm_m
```

The repository holds the assembler only. Two things are deliberately not in git:

- the reference assembler, vasm 1.7h: on macOS/Linux `scripts/get_reference.sh`
  downloads the pinned tarball, verifies its SHA-256 and builds
  `vasm-1.7h/vasmm68k_mot`; on Windows the example project ships
  `vasmm68k_mot.exe`, which is the reference;
- the CubeDroid example project: point the runner at your copy with
  `--project` (or place it at `AssemblyTest/CubeDroid` under the repo root, the
  default).

### Windows

You need: Git for Windows, Python 3 (from python.org, tick "Add to PATH"), and
your existing CubeDroid project folder on the laptop (it contains the reference
assembler at `CubeDroid\Assemblers\vasmm68k_mot.exe`). All commands below are
for PowerShell.

1. Install Rust: run `rustup-init.exe` from https://rustup.rs. If you do not
   have Visual Studio installed, choose **Customize installation** and set the
   default host triple to `x86_64-pc-windows-gnu`; otherwise accept the defaults
   (MSVC, which needs the "Desktop development with C++" build tools). Close and
   reopen PowerShell afterwards so `cargo` is on the PATH.
2. Get and build the assembler:

   ```
   git clone https://github.com/solutionrooms/vasm_m.git
   cd vasm_m
   cargo build --release
   ```

   The binary is `target\release\vasm_m.exe` inside the clone.
3. Compare it with the reference. Replace `C:\path\to\CubeDroid` with your
   project folder (the one containing `SourceCode\stub.X68`):

   ```
   python tests\diff.py --cubedroid --project C:\path\to\CubeDroid --ref C:\path\to\CubeDroid\Assemblers\vasmm68k_mot.exe
   ```

   Expected last lines:

   ```
   pass=68 fail=0
   cubedroid [bin]: ref exit=0 0.5xxs  new exit=0 0.1xxs
   PASS: CubeDroid byte-exact [bin] (944216 bytes)
   ... same for [hunk] and [hunkexe]
   ```

   Anything else is a finding: paste the whole output into `chat.md` or an issue.
4. Build the game with it. `CubeDroid\compile.bat` runs
   `"Assemblers\vasmm68k_mot" -o Output\... -Fbin -spaces SourceCode\stub.X68`.
   Copy `vasm_m.exe` into `CubeDroid\Assemblers\` and change that line to
   `"Assemblers\vasm_m"`; the ROM is identical before the padder runs.
5. Timing, from the CubeDroid folder:

   ```
   Measure-Command { Assemblers\vasm_m.exe -quiet -o Output\test.bin -Fbin -spaces SourceCode\stub.X68 }
   Measure-Command { Assemblers\vasmm68k_mot.exe -quiet -o Output\test.bin -Fbin -spaces SourceCode\stub.X68 }
   ```

   On an M-series Mac this is about 0.11 s against 0.6 s.

`tests\diff.sh` and `tests\cubedroid.sh` are POSIX only; `tests\diff.py` is the
cross-platform runner.

### macOS

```sh
scripts/get_reference.sh                  # vasm 1.7h → vasm-1.7h/vasmm68k_mot (needs clang, make, curl)
cargo build --release
tests/diff.sh && tests/cubedroid.sh       # or: python3 tests/diff.py --cubedroid
```

`cubedroid.sh` expects `AssemblyTest/CubeDroid` under the repo root or `PROJ=<dir>`.
See [docs/usage.md](docs/usage.md) for the supported flags and the fuzzers.

## Build and verify the new assembler

With Rust installed and the reference assembler built as described below:

```sh
cargo build --release
python3 tests/diff.py --cubedroid
```

The new executable is `target/release/vasm_m` (`vasm_m.exe` on Windows).
The comparison suite checks output bytes and expected rejection of invalid input;
a mismatch is a failing test, including known regressions awaiting a fix. Use a
distinct `--out` directory for concurrent test runs. See [docs/usage.md](docs/usage.md)
for host-specific commands and use `chat.md` for current review/fix status.

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
implementation and differential tests before enabling parallel execution. That
baseline exists; additional compatibility work and threading design review remain.

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

Host targets (separate from the generated 68000 code):

| Host | Baseline CubeDroid assembly | Conditions |
| --- | --- | --- |
| macOS, Apple Silicon arm64 | About 0.6 s, locally observed | Power mode not yet recorded |
| Windows x86-64, HPLaptop, Intel Core Ultra 7 258V, 32 GB RAM | About 0.5 s, user-reported | On battery, unplugged |

Windows on battery is a first-class performance target. The Windows measurement
has not yet been independently reproduced; record its exact command and timing
boundary before comparing it with new results. Codex's proposed planning targets
are 100–200 ms for full assembly, with 50–100 ms as a stretch target, on each host.
These are aspirations pending profiling, not predicted or demonstrated results.
Against the Windows baseline those ranges represent 2.5–5x and 5–10x speedups.

Benchmark native release executables against the reference on the same machine,
with identical inputs, options, output validation, and power settings. For Windows
record power mode, battery-saver state and approximate battery charge; do not
substitute plugged-in results for the battery target. Time process start through
output completion, excluding the ROM padder and emulator. Report the first run
separately from repeated warm runs, with median and variation; interleave reference
and candidate runs. Compare thread counts, including one thread, and choose the
default from measured results on both hosts. Cross-host output must also be exact.

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

CubeDroid is the first real workload: one translation unit assembled with `-spaces`
and the selected output format. Include read-ahead was tried and removed after
Claude measured no benefit. The threading architecture remains undecided; batch
parallelism across independent files would not itself speed up this translation
unit. Native Windows runtime verification and battery-mode benchmarks remain pending.
