#!/usr/bin/env python3
"""Regression for the runner path bug (codex P2, 2026-09-10): --ref/--new/--out
given relative to the launch directory must keep working after tests/diff.py
changes cwd for the CubeDroid step. Runs diff.py from the repo root and from
tests/ with relative arguments; on POSIX also runs cubedroid.sh with relative
REF/NEW/OUT. Uses its own output directories so it never races the main runner.

Usage:  python3 tests/check_runner_paths.py
"""
import os, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IS_WIN = os.name == "nt"
EXE = ".exe" if IS_WIN else ""

def check(label, cmd, cwd, env=None):
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, env=env, timeout=600)
    except subprocess.TimeoutExpired:
        print(f"FAIL {label}: timeout"); return False
    ok = p.returncode == 0 and "PASS: CubeDroid byte-exact" in p.stdout
    print(("PASS " if ok else "FAIL ") + label)
    if not ok:
        print(f"  exit={p.returncode}\n" + p.stdout[-2000:] + p.stderr[-2000:])
    return ok

def main():
    py = sys.executable
    ref = "vasm-1.7h/vasmm68k_mot" + EXE
    new = "target/release/vasm_m" + EXE
    ok = True
    ok &= check("diff.py from repo root, relative --ref/--new/--out",
                [py, "tests/diff.py", "--ref", ref, "--new", new,
                 "--out", "target/diff-relpath-root", "--cubedroid"], ROOT)
    ok &= check("diff.py from tests/, ../-relative --ref/--new/--out",
                [py, "diff.py", "--ref", "../" + ref, "--new", "../" + new,
                 "--out", "../target/diff-relpath-sub", "--cubedroid"], ROOT / "tests")
    if not IS_WIN:
        env = dict(os.environ, REF=ref, NEW=new, OUT="target/cubedroid-relpath")
        ok &= check("cubedroid.sh with relative REF/NEW/OUT", ["sh", "tests/cubedroid.sh"], ROOT, env)
    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
