#!/usr/bin/env python3
"""Directive/macro fuzzer: random but well-formed programs mixing labels, data
directives, space/alignment, rs/so offsets, equ/set chains, macros (positional
args, \\@), rept, conditionals, local labels, `*`, and label-referencing
instructions. vasm 1.7h is the oracle; agreement = same exit (0/0 or 1/1) and
identical bytes. Half the files use -spaces.
Usage: python3 tests/fuzz_dir.py [--files N] [--seed S] [--jobs N]
"""
import argparse, os, random, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
REF = ROOT / "vasm-1.7h" / ("vasmm68k_mot" + EXE)
NEW = ROOT / "target" / "release" / ("vasm_m" + EXE)

class Gen:
    def __init__(self, rng, spaces):
        self.r = rng; self.spaces = spaces
        self.labels = []; self.locals = []; self.equs = []; self.sets = []; self.macros = []
        self.n = 0; self.cond_depth = 0; self.out = []

    def fresh(self, p):
        self.n += 1
        return f"{p}{self.n}"

    def sep(self):
        return ", " if (self.spaces and self.r.random() < 0.5) else ","

    def cvalue(self):
        """constant-only value (no labels)"""
        k = self.r.random()
        if k < 0.45: return str(self.r.choice([0, 1, 2, 3, 4, 7, 8, 15, 16, 100, 127, 128, 255, 256, 1000, 32767, 32768, 65535]))
        if k < 0.65: return "$" + format(self.r.randrange(0, 0x10000), "x")
        if k < 0.8 and self.equs: return self.r.choice(self.equs)
        if k < 0.9 and self.sets: return self.r.choice(self.sets)
        return "'" + self.r.choice(["A", "AB", "xyz", "QRST"]) + "'"

    def expr(self, depth=2):
        """constant expression"""
        if depth == 0 or self.r.random() < 0.4: return self.cvalue()
        op = self.r.choice(["+", "-", "*", "|", "&", "<<", ">>"])
        l, r = self.expr(depth - 1), self.expr(depth - 1)
        if op in ("<<", ">>"): r = "(" + r + "&7)"
        if self.spaces and self.r.random() < 0.4: return f"{l} {op} {r}"
        return f"{l}{op}{r}"

    def lexpr(self):
        """label-based (relocatable or label-difference) expression"""
        if not self.labels: return self.expr()
        k = self.r.random()
        if k < 0.4: return self.r.choice(self.labels)
        if k < 0.7: return f"{self.r.choice(self.labels)}+{self.r.randint(0, 16)}"
        if k < 0.85 and len(self.labels) >= 2:
            a, b = self.r.sample(self.labels, 2); return f"({a}-{b})"
        return f"{self.r.choice(self.labels)}-{self.r.randint(0, 16)}"

    def value(self):
        return self.cvalue()

    def emit(self, s): self.out.append(s)

    def label(self, local=False):
        if local and self.labels:
            name = "." + self.fresh("l")
            self.locals.append(name)
            self.emit(f"{name}{':' if self.r.random() < 0.7 else ''}")
        else:
            name = self.fresh("L")
            self.labels.append(name); self.locals = []
            self.emit(f"{name}{':' if self.r.random() < 0.6 else ''}")

    def data(self):
        d = self.r.choice(["dc.b", "dc.w", "dc.l", "dc"])
        items = []
        for _ in range(self.r.randint(1, 6)):
            if d == "dc.b" and self.r.random() < 0.3:
                items.append(self.r.choice(["'hello'", "'a''b'", "\"str\"", "'x'", "''"]))
            elif d == "dc.l" and self.r.random() < 0.4:
                items.append(self.lexpr())
            elif d != "dc.b" and len(self.labels) >= 2 and self.r.random() < 0.2:
                a, b = self.r.sample(self.labels, 2); items.append(f"{a}-{b}")
            else:
                e = self.expr()
                if d == "dc.b": e = f"({e})&$ff"
                elif d in ("dc.w", "dc"): e = f"({e})&$ffff"
                items.append(e)
        self.emit(f"\t{d}\t{self.sep().join(items)}")

    def space(self):
        k = self.r.random()
        if k < 0.3: self.emit(f"\tds.{self.r.choice('bwl')}\t{self.r.randint(0, 9)}")
        elif k < 0.5: self.emit(f"\tdcb.{self.r.choice('bwl')}\t{self.r.randint(0, 5)}{self.sep()}{self.r.choice(['0', '$ff', '$1234', '-1', '$a5'])}")
        elif k < 0.65: self.emit("\teven")
        elif k < 0.8:
            a = self.r.choice([2, 4, 8, 16, 32]); self.emit(f"\tcnop\t{self.r.randint(0, a - 1) if self.r.random() < 0.3 else 0}{self.sep()}{a}")
        elif k < 0.9: self.emit(f"\talign\t{self.r.randint(0, 4)}")
        else: self.emit(f"\tds.b\t{self.r.choice(['1', '3', '5', '(3&2)'])}")

    def equ(self):
        name = self.fresh("E")
        if self.r.random() < 0.15 and len(self.labels) >= 2:
            a, b = self.r.sample(self.labels, 2)
            self.emit(f"{name}\t{self.r.choice(['equ', 'EQU', '='])}\t{a}-{b}")
        else:
            self.emit(f"{name}\t{self.r.choice(['equ', 'EQU', '='])}\t{self.expr()}")
        self.equs.append(name)

    def setsym(self):
        if self.sets and self.r.random() < 0.6:
            name = self.r.choice(self.sets)
            self.emit(f"{name}\tset\t{name}+{self.r.randint(1, 8)}")
        else:
            name = self.fresh("S"); self.emit(f"{name}\tset\t{self.expr()}"); self.sets.append(name)

    def rs(self):
        k = self.r.random()
        if k < 0.2: self.emit("\trsreset" if self.r.random() < 0.5 else "\tclrso")
        elif k < 0.3: self.emit(f"\trsset\t{self.r.randint(0, 100)}")
        else:
            name = self.fresh("R"); self.emit(f"{name}\t{self.r.choice(['rs.b', 'rs.w', 'rs.l', 'so.b', 'so.w', 'so.l', 'rs'])}\t{self.r.randint(0, 4)}"); self.equs.append(name)

    def macro_def(self):
        name = self.fresh("M")
        nargs = self.r.randint(0, 3)
        self.emit(f"{name}\tmacro")
        body = self.r.randint(1, 4)
        for _ in range(body):
            k = self.r.random()
            if k < 0.3 and nargs: self.emit(f"\tdc.w\t(\\{self.r.randint(1, nargs)})&$ffff")
            elif k < 0.45: self.emit(f"\\@x:\tnop")
            elif k < 0.6: self.emit("\tbra.s\t\\@x" if any("\\@x" in l for l in self.out[-3:]) else "\tnop")
            elif k < 0.75 and nargs: self.emit(f"\tmove.w\t#\\{self.r.randint(1, nargs)},d{self.r.randint(0, 7)}")
            elif k < 0.85: self.emit(f"\tds.b\t{self.r.randint(0, 3)}")
            else: self.emit("\tnop")
        self.emit("\tendm")
        self.macros.append((name, nargs))

    def macro_call(self):
        if not self.macros: return self.macro_def()
        name, nargs = self.r.choice(self.macros)
        args = [self.r.choice([str(self.r.randint(0, 200)), "$1f", "7", "'A'"]) for _ in range(nargs)]
        self.emit(f"\t{name}\t{self.sep().join(args)}")

    def rept(self):
        n = self.r.randint(0, 4)
        self.emit(f"\trept\t{n}")
        self.emit(f"\t{self.r.choice(['nop', 'dc.b 1', 'dc.w $abcd', 'ds.b 1'])}")
        self.emit("\tendr")

    def cond(self):
        c = self.r.choice([f"ifd {self.r.choice(self.equs) if self.equs else 'NOPE'}", "ifnd NOPE", f"ifeq {self.r.randint(0, 1)}", f"ifne {self.r.randint(0, 1)}", "ifgt 1", "iflt 1"])
        self.emit(f"\t{c}")
        self.emit(f"\tdc.w\t{self.r.randint(0, 255)}")
        if self.r.random() < 0.5:
            self.emit("\telse"); self.emit("\tnop")
        self.emit("\tendif" if self.r.random() < 0.7 else "\tendc")

    def inst(self):
        k = self.r.random()
        tgt = self.r.choice(self.locals) if (self.locals and self.r.random() < 0.4) else (self.r.choice(self.labels) if self.labels else None)
        if tgt is None or k < 0.3:
            self.emit(f"\t{self.r.choice(['nop', 'rts', 'moveq #1,d0', 'move.l d0,d1', 'clr.w d2', 'addq.l #1,a0', 'move.w #$1234,d3', 'lea 4(a0),a1'])}")
        elif k < 0.55: self.emit(f"\t{self.r.choice(['bra', 'bsr', 'beq', 'bne', 'bra.s', 'bra.w', 'dbf d0,', 'jmp', 'jsr'])}{'' if k > 0.5 else ''}\t{tgt}".replace(",\t", ","))
        elif k < 0.7: self.emit(f"\tlea\t{tgt}{self.r.choice(['', '(pc)'])},a{self.r.randint(0, 6)}")
        elif k < 0.85: self.emit(f"\tmove.{self.r.choice('wl')}\t{tgt}{self.r.choice(['', '(pc)'])},d{self.r.randint(0, 7)}")
        else: self.emit(f"\tmove.l\t#{tgt}{self.r.choice(['', '+2', '-' + (self.labels[0] if self.labels else '0')])},d{self.r.randint(0, 7)}")
        # (a local label difference stays relocatable-safe: same section)

    def star(self):
        self.emit(f"\tdc.l\t*" if self.r.random() < 0.5 else f"{self.fresh('P')}\tequ\t*-{self.labels[0] if self.labels else '0'}")

    def gen(self, lines):
        self.emit("\tsection code,code" if self.r.random() < 0.3 else ";; fuzz")
        for _ in range(lines):
            self.r.choice([self.label, self.label, self.data, self.data, self.space, self.equ, self.setsym, self.rs,
                           self.macro_def, self.macro_call, self.rept, self.cond, self.inst, self.inst, self.star,
                           lambda: self.label(True)])()
        return "\n".join(self.out) + "\n"

def run_one(args):
    idx, text, spaces, tmp = args
    src = tmp / f"d{idx:04d}.s"
    src.write_text(text)
    flags = ["-quiet", f"-F{FORMAT}", "-m68000"] + (["-spaces"] if spaces else []) + FLAGS
    rf, nf = tmp / f"d{idx:04d}.ref", tmp / f"d{idx:04d}.out"
    try:
        r = subprocess.run([str(REF), *flags, "-o", str(rf), str(src)], capture_output=True, text=True, timeout=20)
        n = subprocess.run([str(NEW), *flags, "-o", str(nf), str(src)], capture_output=True, text=True, timeout=20)
    except subprocess.TimeoutExpired as e:
        return f"{src}: timeout {e.cmd[0]}"
    if not ((r.returncode == 0 and n.returncode == 0) or (r.returncode == 1 and n.returncode == 1)):
        return f"{src}: exit ref={r.returncode} new={n.returncode}\n ref: {r.stderr[:300]}\n new: {n.stderr[:300]}"
    if r.returncode == 0 and rf.read_bytes() != nf.read_bytes():
        rb, nb = rf.read_bytes(), nf.read_bytes()
        first = next((i for i, (x, y) in enumerate(zip(rb, nb)) if x != y), min(len(rb), len(nb)))
        return f"{src}: bytes differ at {first:#x} ({len(rb)} vs {len(nb)})"
    for f in (src, rf, nf):
        if f.exists(): f.unlink()
    return "OK" if r.returncode == 0 else "REJECTED"

FORMAT = "bin"
FLAGS = []

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--files", type=int, default=500)
    ap.add_argument("--lines", type=int, default=40)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--format", default="bin", help="output format to compare (bin, hunk, hunkexe)")
    ap.add_argument("--flags", default="", help="extra flags for both assemblers, e.g. '-databss' (use --flags=-x form)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    a = ap.parse_args()
    global FORMAT, FLAGS
    FORMAT = a.format
    FLAGS = a.flags.split()
    rng = random.Random(a.seed)
    tmp = Path(tempfile.mkdtemp(prefix="vasm_m_fdir_"))
    jobs = []
    for i in range(a.files):
        spaces = i % 2 == 0
        jobs.append((i, Gen(random.Random(rng.random()), spaces).gen(a.lines), spaces, tmp))
    fails = []; rejected = 0; accepted = 0
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        for res in ex.map(run_one, jobs):
            if res == "OK": accepted += 1
            elif res == "REJECTED": rejected += 1
            elif res: fails.append(res)
    for f in fails[:25]: print("MISMATCH", f)
    print(f"files={a.files} assembled={accepted} rejected-by-both={rejected} mismatches={len(fails)} (kept in {tmp})")
    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
