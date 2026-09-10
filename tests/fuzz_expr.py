#!/usr/bin/env python3
"""Expression fuzzer: random constant expressions in the mot syntax grammar,
emitted as `dc.l`, `dc.w`, `dc.b` and `equ` lines; vasm 1.7h is the oracle.
Exercises precedence (<< >> bind tighter than & ^ | which bind tighter than * /),
BOOLEAN(x) = -1, signed div/mod, arithmetic >>, char constants, number bases,
and -spaces vs no-spaces operand termination.
Usage: python3 tests/fuzz_expr.py [--files N] [--lines N] [--seed S] [--jobs N]
"""
import argparse, os, random, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
REF = ROOT / "vasm-1.7h" / ("vasmm68k_mot" + EXE)
NEW = ROOT / "target" / "release" / ("vasm_m" + EXE)

BIN = ["+", "-", "*", "/", "//", "%", "&", "|", "^", "~", "!", "<<", ">>", "<", ">", "<=", ">=", "=", "==", "!=", "<>", "&&", "||"]
UN = ["-", "~", "!", "+"]

def num(rng):
    k = rng.random()
    if k < 0.35: return str(rng.choice([0, 1, 2, 3, 7, 8, 15, 16, 31, 32, 100, 255, 256, 4095, 65535, 65536, 2147483647]))
    if k < 0.6: return "$" + format(rng.choice([0, 1, 0x7f, 0x80, 0xff, 0x100, 0x7fff, 0x8000, 0xffff, 0x10000, 0xdead, 0x7fffffff, 0x80000000, 0xffffffff, 0xfffff000]), "x")
    if k < 0.7: return "%" + format(rng.randrange(0, 256), "b")
    if k < 0.75: return "@" + format(rng.randrange(0, 512), "o")
    if k < 0.9: return "'" + rng.choice(["A", "AB", "ABC", "ABCD", "'", "z"]).replace("'", "''") + "'"
    return rng.choice(["K1", "K2", "K3"])

def expr(rng, depth):
    if depth <= 0 or rng.random() < 0.25:
        return num(rng)
    k = rng.random()
    if k < 0.15:
        return rng.choice(UN) + expr(rng, depth - 1)
    if k < 0.3:
        return "(" + expr(rng, depth - 1) + ")"
    op = rng.choice(BIN)
    left, right = expr(rng, depth - 1), expr(rng, depth - 1)
    if op in ("/", "//", "%"):
        right = "(" + right + "|1)"          # avoid division by zero
    if op in ("<<", ">>"):
        right = "(" + right + "&31)"          # keep shift counts defined
    return left + op + right

def make_file(rng, lines, spaces):
    out = ["K1\tequ\t$1234", "K2\tequ\t-7", "K3\tequ\t'AB'"]
    for i in range(lines):
        e = expr(rng, rng.randint(1, 5))
        if spaces and rng.random() < 0.3:
            e = e.replace("+", " + ").replace("*", " * ")
        d = rng.choice(["dc.l", "dc.l", "dc.w", "dc.b", "equ"])
        if d == "equ":
            out.append(f"E{i}\tequ\t{e}")
            out.append(f"\tdc.l\tE{i}")
        elif d == "dc.w":
            out.append(f"\tdc.w\t({e})&$ffff")
        elif d == "dc.b":
            out.append(f"\tdc.b\t({e})&$ff")
        else:
            out.append(f"\tdc.l\t{e}")
    return "\n".join(out) + "\n"

def run_one(args):
    idx, text, spaces, tmp = args
    src = tmp / f"e{idx:04d}.s"
    src.write_text(text)
    flags = ["-quiet", "-Fbin", "-m68000"] + (["-spaces"] if spaces else [])
    rf, nf = tmp / f"e{idx:04d}.ref", tmp / f"e{idx:04d}.out"
    r = subprocess.run([str(REF), *flags, "-o", str(rf), str(src)], capture_output=True, text=True)
    n = subprocess.run([str(NEW), *flags, "-o", str(nf), str(src)], capture_output=True, text=True)
    if (r.returncode == 0) != (n.returncode == 0):
        return f"{src}: exit ref={r.returncode} new={n.returncode}\n ref: {r.stderr[:300]}\n new: {n.stderr[:300]}"
    if r.returncode == 0 and rf.read_bytes() != nf.read_bytes():
        rb, nb = rf.read_bytes(), nf.read_bytes()
        first = next((i for i, (x, y) in enumerate(zip(rb, nb)) if x != y), min(len(rb), len(nb)))
        return f"{src}: bytes differ at {first:#x} (ref {rb[first:first+4].hex()} new {nb[first:first+4].hex()})"
    for f in (src, rf, nf):
        if f.exists(): f.unlink()
    return None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--files", type=int, default=400)
    ap.add_argument("--lines", type=int, default=60)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    a = ap.parse_args()
    rng = random.Random(a.seed)
    tmp = Path(tempfile.mkdtemp(prefix="vasm_m_fexpr_"))
    jobs = [(i, make_file(rng, a.lines, i % 2 == 0), i % 2 == 0, tmp) for i in range(a.files)]
    fails = []
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        for res in ex.map(run_one, jobs):
            if res: fails.append(res)
    for f in fails[:20]: print("MISMATCH", f)
    print(f"files={a.files} lines/file={a.lines} mismatches={len(fails)} (kept in {tmp})")
    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
