#!/usr/bin/env python3
"""Generated stress corpus: every 68000-eligible mnemonic row x size suffixes x
sampled operands, one instruction per file, vasm 1.7h as the oracle.

A case passes when both assemblers agree: same exit status and, on success,
identical bytes. Cases where the reference fails are expected rejections.
Usage: python3 tests/fuzz.py [--jobs N] [--limit N] [--keep DIR] [--seed S]
"""
import argparse, itertools, os, random, re, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
REF = ROOT / "vasm-1.7h" / ("vasmm68k_mot" + EXE)
NEW = ROOT / "target" / "release" / ("vasm_m" + EXE)
M68000 = 0x1

def load_tables():
    src = (ROOT / "src" / "m68k" / "tables.rs").read_text()
    consts = {m.group(1): int(m.group(2)) for m in re.finditer(r"pub const (\w+): u8 = (\d+);", src)}
    names = {v: k for k, v in consts.items() if not k.startswith("PL_")}
    rows = []
    for m in re.finditer(r'Mnemonic \{ name: "([^"]*)", operand_type: \[([^\]]*)\], place: \[[^\]]*\], opcode: \[[^\]]*\], size: (0x[0-9a-f]+), available: (0x[0-9a-f]+) \}', src):
        name, ops, size, avail = m.group(1), [int(x) for x in m.group(2).split(",")], int(m.group(3), 16), int(m.group(4), 16)
        if not name or name.startswith(" ") or avail & M68000 == 0:
            continue
        rows.append((name, [names[o] for o in ops if o], size))
    return rows

EA_ALL = ["d0", "d7", "a0", "a6", "sp", "(a0)", "(sp)", "(a1)+", "-(a2)", "4(a0)", "0(a0)", "-1(a0)", "32767(a0)", "-32768(a0)",
          "(a0,d0.w)", "4(a0,d1.l)", "127(a0,a1.w)", "-128(a0,d0)", "lab_b", "lab_f", "lab_b(pc)", "4(pc,d0)", "lab_f(pc,d1.w)",
          "$1000", "$1000.w", "$8000", "$100000", "$abcd.l", "$ffff8000", "#5", "#lab_f", "#-1", "#$ffff", "#$12345678", "sr", "ccr", "usp"]
SAMPLES = {
    "D_": ["d0", "d7"], "A_": ["a0", "a6", "sp"], "AI": ["(a0)", "(sp)"], "R_": ["d1", "a1"],
    "PA": ["-(a0)", "-(sp)"], "AP": ["(a0)+", "(sp)+"], "DP": ["4(a0)", "(a1)", "-3(a2)"],
    "IM": ["#1", "#$7f", "#$ff", "#$100", "#$8000", "#-1", "#$12345678", "#lab_f", "#-129", "#$ffff"],
    "QI": ["#1", "#8", "#0", "#9", "#-1", "#4"], "IR": ["#$0f0f", "#$8001"],
    "BR": ["lab_b", "lab_f", "far_f", "far_b"], "VA": ["#$123", "$fff"],
    "RL": ["d0-d7/a0-a6", "d0", "a0/d3", "d0-d3", "a0-a2/a4/d0-d7", "d7/a1/a2/a5", "d0-7"],
    "_CCR": ["ccr"], "_SR": ["sr"], "_USP": ["usp"],
}
def samples(ot):
    return SAMPLES.get(ot, EA_ALL)

def gen_cases(rows, rng, limit):
    cases = []
    for name, ops, size in rows:
        sizes = [""]
        if size & 0x7f00:
            sizes += [".b", ".w", ".l", ".s"]
        else:
            sizes += [".w"]
        for sz in sizes:
            if not ops:
                cases.append(f"\t{name}{sz}")
                continue
            lists = [samples(o) for o in ops]
            combos = list(itertools.product(*lists))
            if len(combos) > 24:
                combos = rng.sample(combos, 24)
            for c in combos:
                cases.append(f"\t{name}{sz}\t{','.join(c)}")
    rng.shuffle(cases)
    return cases[:limit] if limit else cases

TEMPLATE = """far_b:\tnop
\tds.b\t200
lab_b:\tnop
{}
\tnop
lab_f:\tnop
\tds.b\t200
far_f:\tnop
"""

FLAGS = []
TIMEOUT = 20

def run_case(args):
    idx, line, tmp = args
    src = tmp / f"c{idx:06d}.s"
    src.write_text(TEMPLATE.format(line))
    rf, nf = tmp / f"c{idx:06d}.ref", tmp / f"c{idx:06d}.out"
    try:
        r = subprocess.run([str(REF), "-quiet", "-Fbin", "-m68000", *FLAGS, "-o", str(rf), str(src)], capture_output=True, text=True, timeout=TIMEOUT)
        n = subprocess.run([str(NEW), "-quiet", "-Fbin", "-m68000", *FLAGS, "-o", str(nf), str(src)], capture_output=True, text=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired as e:
        return (line, f"timeout: {e.cmd[0]}")
    rs, ns = r.returncode, n.returncode
    # agreement: same success, or reference rejection (1) matched by rejection (1);
    # any other code (crash, signal, panic) is a failure
    if not ((rs == 0 and ns == 0) or (rs == 1 and ns == 1)):
        return (line, f"exit ref={rs} new={ns}\n  ref: {r.stderr.strip()[:200]}\n  new: {n.stderr.strip()[:200]}")
    if rs == 0:
        rb, nb = rf.read_bytes(), nf.read_bytes()
        if rb != nb:
            return (line, f"bytes differ: ref={rb.hex()} new={nb.hex()}")
    for f in (src, rf, nf):
        if f.exists(): f.unlink()
    return None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--keep", default=None)
    ap.add_argument("--flags", default="", help="extra flags for both assemblers, e.g. '-no-opt'")
    a = ap.parse_args()
    if not REF.is_file() or not NEW.is_file():
        sys.exit("build both assemblers first")
    global FLAGS
    FLAGS = a.flags.split()
    rows = load_tables()
    cases = gen_cases(rows, random.Random(a.seed), a.limit)
    tmp = Path(a.keep) if a.keep else Path(tempfile.mkdtemp(prefix="vasm_m_fuzz_"))
    tmp.mkdir(parents=True, exist_ok=True)
    print(f"{len(rows)} 68000 rows, {len(cases)} cases, workdir {tmp}")
    fails = []
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        for res in ex.map(run_case, [(i, c, tmp) for i, c in enumerate(cases)]):
            if res:
                fails.append(res)
    fails.sort()
    for line, why in fails[:80]:
        print(f"MISMATCH {line.strip()}\n  {why}")
    print(f"cases={len(cases)} mismatches={len(fails)}")
    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
