#!/usr/bin/env python3
"""Cross-platform differential test (mirrors tests/diff.sh and tests/cubedroid.sh).

Usage:  python3 tests/diff.py [--ref PATH] [--new PATH] [--out DIR] [--formats "bin hunk"] [--cubedroid] [--corpus-only]

Paths may be relative to the directory you launch from. Concurrent runs must use
distinct --out directories (the default target/diff is shared).

Positive corpus cases (tests/corpus/NAME.s) must exit 0 on both assemblers and
produce identical bytes; negative cases (NAME.expect-fail present) must exit 1 on
both. NAME.flags adds flags to both; NAME.<fmt>.flags / NAME.<fmt>.expect-fail apply
to one format; NAME.formats lists the formats for that case. Exit status is
non-zero on any failure.
"""
import argparse, os, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IS_WIN = os.name == "nt"
EXE = ".exe" if IS_WIN else ""

def run(cmd, cwd=None, timeout=60):
    t0 = time.perf_counter()
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return 124, f"timeout after {timeout}s", time.perf_counter() - t0
    return p.returncode, p.stdout + p.stderr, time.perf_counter() - t0

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", default=str(ROOT / "vasm-1.7h" / ("vasmm68k_mot" + EXE)))
    ap.add_argument("--new", default=str(ROOT / "target" / "release" / ("vasm_m" + EXE)))
    ap.add_argument("--formats", default="bin hunk")
    ap.add_argument("--out", default=str(ROOT / "target" / "diff"))
    ap.add_argument("--cubedroid", action="store_true", help="also run the full-ROM gate")
    ap.add_argument("--corpus-only", action="store_true")
    a = ap.parse_args()
    # Resolve every user-supplied path against the launch directory before any
    # subprocess runs with a different cwd (the CubeDroid step uses cwd=proj).
    ref, new = Path(a.ref).resolve(), Path(a.new).resolve()
    if not ref.is_file():
        sys.exit(f"reference vasm not built: {ref}")
    if not new.is_file():
        sys.exit(f"vasm_m not built: {new} (cargo build --release)")
    out = Path(a.out).resolve() / "corpus"
    if out.exists():
        for f in out.iterdir():
            if f.is_file():
                f.unlink()
    out.mkdir(parents=True, exist_ok=True)
    common = ["-quiet", "-m68000"]
    srcs = sorted((ROOT / "tests" / "corpus").glob("*.s"))
    if not srcs:
        sys.exit("corpus is empty")
    passed = failed = 0
    for src in srcs:
        name = src.stem
        flags_file = src.with_suffix(".flags")
        cflags = flags_file.read_text().split() if flags_file.exists() else []
        fmts_file = src.with_suffix(".formats")
        formats = fmts_file.read_text().split() if fmts_file.exists() else a.formats.split()
        for fmt in formats:
            ff = src.with_suffix(f".{fmt}.flags")
            flags = cflags + (ff.read_text().split() if ff.exists() else [])
            negative = src.with_suffix(".expect-fail").exists() or src.with_suffix(f".{fmt}.expect-fail").exists()
            rf, nf = out / f"{name}.{fmt}.ref", out / f"{name}.{fmt}.out"
            rs, rlog, _ = run([str(ref), *common, f"-F{fmt}", *flags, "-o", str(rf), str(src)])
            ns, nlog, _ = run([str(new), *common, f"-F{fmt}", *flags, "-o", str(nf), str(src)])
            ok = True
            if negative:
                if rs != 1:
                    print(f"FAIL {name} [{fmt}]: negative case but reference exited {rs}"); ok = False
                elif ns != 1:
                    print(f"FAIL {name} [{fmt}]: vasm_m exited {ns}, expected rejection (1)\n{nlog}"); ok = False
            else:
                if rs != 0:
                    print(f"FAIL {name} [{fmt}]: reference exited {rs} on a positive case\n{rlog}"); ok = False
                elif ns != 0:
                    print(f"FAIL {name} [{fmt}]: vasm_m exited {ns}\n{nlog}"); ok = False
                elif not nf.exists():
                    print(f"FAIL {name} [{fmt}]: vasm_m produced no output file"); ok = False
                else:
                    rb, nb = rf.read_bytes(), nf.read_bytes()
                    if rb != nb:
                        first = next((i for i, (x, y) in enumerate(zip(rb, nb)) if x != y), min(len(rb), len(nb)))
                        print(f"FAIL {name} [{fmt}]: bytes differ ({len(rb)} vs {len(nb)}), first at offset {first:#x}"); ok = False
            if ok: passed += 1
            else: failed += 1
    print(f"pass={passed} fail={failed}")
    if a.cubedroid and not a.corpus_only:
        proj = ROOT / "AssemblyTest" / "CubeDroid"
        for fmt in ("bin", "hunk", "hunkexe"):
            rf, nf = out / f"cubedroid.ref.{fmt}", out / f"cubedroid.new.{fmt}"
            rs, rlog, rt = run([str(ref), "-quiet", f"-F{fmt}", "-spaces", "-o", str(rf), "SourceCode/stub.X68"], cwd=proj)
            ns, nlog, nt = run([str(new), "-quiet", f"-F{fmt}", "-spaces", "-o", str(nf), "SourceCode/stub.X68"], cwd=proj)
            print(f"cubedroid [{fmt}]: ref exit={rs} {rt:.3f}s  new exit={ns} {nt:.3f}s")
            if rs != 0:
                print(f"FAIL [{fmt}]: reference failed\n" + rlog); failed += 1
            elif ns != 0:
                print(f"FAIL [{fmt}]: vasm_m failed\n" + nlog); failed += 1
            elif rf.read_bytes() != nf.read_bytes():
                rb, nb = rf.read_bytes(), nf.read_bytes()
                first = next((i for i, (x, y) in enumerate(zip(rb, nb)) if x != y), min(len(rb), len(nb)))
                print(f"FAIL [{fmt}]: CubeDroid differs ({len(rb)} vs {len(nb)}), first at offset {first:#x}"); failed += 1
            else:
                print(f"PASS: CubeDroid byte-exact [{fmt}] ({len(rf.read_bytes())} bytes)")
    sys.exit(1 if failed else 0)

if __name__ == "__main__":
    main()
