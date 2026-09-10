# vasm_m — building and using

vasm_m is a drop-in replacement for `vasmm68k_mot` (vasm 1.7h) for plain 68000
code with `-Fbin` output. Same flags, same bytes, about 4–5× faster.

## Build

Requires a Rust toolchain (https://rustup.rs; 1.85+ / edition 2024).

    cargo build --release
    # binary: target/release/vasm_m   (Windows: target\release\vasm_m.exe)

## Use

Exactly as vasm, e.g. CubeDroid's `compile.bat` line becomes:

    vasm_m -o Output/CubeDroid.bin -Fbin -spaces SourceCode/stub.X68

Supported: `-Fbin`, `-o`, `-quiet`, `-spaces`, `-m68000`, `-Ipath`, `-Dsym[=val]`,
`-no-opt`, `-opt-*`, `-maxerrors=n`, `-nowarn=n`, `-w`, `-x`, `-nocase`,
`-unsshift`, `-esc`, `-allmp`, `-ldots`, `-localu`, `-align`, `-ignore-mult-inc`,
`-regsymredef`. `-L` is accepted but no listing is written. Anything 68010+/
ColdFire/FPU, `-Fhunk`/`-Felf`, `-devpac`/`-phxass` are rejected.

Exit status: 0 on success, 1 on any error (output file removed), like vasm.

## Verify against the reference

    make -C vasm-1.7h -f Makefile.macOS CPU=m68k SYNTAX=mot   # reference build
    tests/diff.sh && tests/cubedroid.sh      # POSIX
    python3 tests/diff.py --cubedroid        # any OS
    python3 tests/fuzz.py                    # 19.5k generated instructions
    python3 tests/fuzz_expr.py               # random expressions
    python3 tests/fuzz_dir.py                # random directive/macro programs

## Timing

`VASM_M_TIMING=1 vasm_m ...` prints phase times to stderr.
