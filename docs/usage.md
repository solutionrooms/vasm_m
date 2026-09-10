# vasm_m — building and using

vasm_m is a drop-in replacement for `vasmm68k_mot` (vasm 1.7h) for plain 68000
code with `-Fbin`, `-Fhunk` (AmigaOS object) and `-Fhunkexe` (AmigaOS
executable) output. Same flags, same bytes, about 4–5× faster.

## Build

Requires a Rust toolchain (https://rustup.rs; 1.85+ / edition 2024).

    cargo build --release
    # binary: target/release/vasm_m   (Windows: target\release\vasm_m.exe)

## Use

Exactly as vasm, e.g. CubeDroid's `compile.bat` line becomes:

    vasm_m -o Output/CubeDroid.bin -Fbin -spaces SourceCode/stub.X68

Supported: `-Fbin`, `-Fhunk`, `-Fhunkexe`, `-o`, `-quiet`, `-spaces`, `-m68000`,
`-Ipath`, `-Dsym[=val]`, `-no-opt`, `-opt-*`, `-maxerrors=n`, `-nowarn=n`, `-w`,
`-x`, `-nosym`, `-nocase`, `-unsshift`, `-esc`, `-allmp`, `-ldots`, `-localu`,
`-align`, `-ignore-mult-inc`, `-regsymredef`. Hunk options: `-kick1hunks`,
`-linedebug`, `-keepempty`, and `-databss` (hunkexe only). `-L` is accepted but
no listing is written. Anything 68010+/ColdFire/FPU, `-Felf`, `-devpac`/`-phxass`
are rejected.

Exit status: 0 on success, 1 on any error (output file removed), like vasm.

## Verify against the reference

    make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot   # reference build
    tests/diff.sh && tests/cubedroid.sh      # POSIX: corpus in bin+hunk, CubeDroid in bin/hunk/hunkexe
    python3 tests/diff.py --cubedroid        # any OS, same gate
    python3 tests/fuzz.py [--format=hunk]    # 19.5k generated instructions
    python3 tests/fuzz_expr.py               # random expressions
    python3 tests/fuzz_dir.py [--format=hunkexe]  # random directive/macro programs
    python3 tests/check_runner_paths.py      # runners accept relative paths

Corpus conventions (`tests/corpus/`): `NAME.flags` (all formats), `NAME.<fmt>.flags`,
`NAME.expect-fail` / `NAME.<fmt>.expect-fail`, `NAME.formats` (formats for that case).

## Timing

`VASM_M_TIMING=1 vasm_m ...` prints phase times to stderr.
