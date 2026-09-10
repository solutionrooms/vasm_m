#!/usr/bin/env python3
"""Transliterate vasm-1.7h/cpus/m68k/{cpu.h,operands.h,opcodes.h,specregs.h}
into src/m68k/tables.rs. Row order and indices are preserved exactly."""
import re, sys, os
ROOT = os.path.join(os.path.dirname(__file__), '..', 'vasm-1.7h', 'cpus', 'm68k')
def rd(n): return open(os.path.join(ROOT, n)).read()
cpu_h, operands_h, opcodes_h, specregs_h = rd('cpu.h'), rd('operands.h'), rd('opcodes.h'), rd('specregs.h')

# ---- macros from cpu.h ------------------------------------------------------
macros = {}
cpu_h_joined = re.sub(r'\\\n', ' ', cpu_h)
cpu_h_joined = re.sub(r'/\*.*?\*/', '', cpu_h_joined, flags=re.S)
for m in re.finditer(r'^#define\s+([A-Za-z_]\w*)\s+(.+?)\s*$', cpu_h_joined, re.M):
    name, val = m.group(1), m.group(2).strip()
    if '(' in name: continue
    macros[name] = val
def ev(expr, extra=None):
    env = {}
    env.update(extra or {})
    e = expr
    for _ in range(20):
        prev = e
        def sub(m):
            n = m.group(0)
            if extra and n in extra: return str(extra[n])
            if n in macros: return '(' + macros[n] + ')'
            return n
        e = re.sub(r'[A-Za-z_]\w*', sub, e)
        if e == prev: break
    e = e.replace('~0', '0xffffffff')
    return eval(e) & 0xffffffff
# ---- operand type enum ------------------------------------------------------
m = re.search(r'enum\s*\{\s*(OP_D8=1.*?)\};', operands_h, re.S)
optnames = []
for tok in re.split(r'[,\s]+', m.group(1).strip()):
    if not tok: continue
    tok = tok.split('=')[0]
    optnames.append(tok)
optype_idx = {n: i+1 for i, n in enumerate(optnames)}  # OP_D8=1
# ---- optypes table ----------------------------------------------------------
body = re.search(r'struct optype optypes\[\] = \{(.*?)\};', operands_h, re.S).group(1)
body = re.sub(r'/\*.*?\*/', '', body, flags=re.S)
rows = [r.strip() for r in re.split(r'\n\s*\n|,\s*\n', body) if r.strip()]
# rows may split incorrectly; instead parse sequentially: each row = "_(16 args),flags,first,last" or "0,0,0,0"
optypes = []
pos = 0
# special register enum (REG_CCR=0 ...), skipping #if 0 blocks
em = re.search(r'enum\s*\{\s*(REG_CCR=0.*?)\};', operands_h, re.S).group(1)
em = re.sub(r'#if 0.*?#endif', '', em, flags=re.S)
regenum = {}
for i, tok in enumerate([t.split('=')[0] for t in re.split(r'[,\s]+', em.strip()) if t]):
    regenum[tok] = i
def fl(v):
    v = v.strip()
    return regenum[v] if v in regenum else int(v)
pat = re.compile(r'\s*(?:_\(([\d,\s]+)\)|(\d+))\s*,\s*([^,]+?)\s*,\s*(-?\w+)\s*,\s*(-?\w+)\s*,?', re.S)
for mm in pat.finditer(body):
    if mm.group(1):
        bits = [int(x) for x in mm.group(1).split(',')]
        modes = sum(b << i for i, b in enumerate(bits))
    else:
        modes = int(mm.group(2))
    flags = ev(mm.group(3))
    optypes.append((modes, flags, fl(mm.group(4)), fl(mm.group(5))))
assert len(optypes) == len(optnames) + 1, (len(optypes), len(optnames))
# ---- place enum + insert_info ---------------------------------------------
m = re.search(r'enum\s*\{\s*(NOP=0.*?)\};', operands_h, re.S)
placenames = [t.split('=')[0] for t in re.split(r'[,\s]+', m.group(1).strip()) if t]
place_idx = {n: i for i, n in enumerate(placenames)}
body = re.search(r'struct oper_insert insert_info\[\] = \{(.*?)\};', operands_h, re.S).group(1)
body = re.sub(r'/\*.*?\*/', '', body, flags=re.S)
inserts = []
for mm in re.finditer(r'(M_\w+)\s*,\s*(0x[0-9a-fA-F]+|\d+)\s*,\s*(\d+)\s*,\s*([^,]+?)\s*,\s*(\w+)\s*,?', body):
    mode = ev(mm.group(1)); size = int(mm.group(2), 0); pos_ = int(mm.group(3)); flags = ev(mm.group(4)); func = mm.group(5)
    inserts.append((mode, size, pos_, flags, func))
assert len(inserts) == len(placenames), (len(inserts), len(placenames))
# ---- mnemonics --------------------------------------------------------------
mn = []
rowpat = re.compile(r'"([^"]*)"\s*,\s*\{([^}]*)\}\s*,\s*\{\{([^}]*)\}\s*,\s*\{([^}]*)\}\s*,\s*([^,]+?)\s*,\s*([^}]+?)\}\s*,?', re.S)
src = re.sub(r'/\*.*?\*/', '', opcodes_h, flags=re.S)
for mm in rowpat.finditer(src):
    name = mm.group(1)
    ops = [t.strip() for t in mm.group(2).split(',') if t.strip()]
    ops = [0 if t == '0' else optype_idx[t] for t in ops]
    places = [t.strip() for t in mm.group(3).split(',') if t.strip()]
    places = [0 if t == '0' else place_idx[t] for t in places]
    opc = [int(t.strip(), 0) for t in mm.group(4).split(',')]
    size = ev(mm.group(5))
    avail = ev(mm.group(6))
    mn.append((name, ops, places, opc, size, avail))
# ---- specregs ---------------------------------------------------------------
regs = []
specregs_clean = re.sub(r'#if 0.*?#endif', '', specregs_h, flags=re.S)
specregs_clean = re.sub(r'/\*.*?\*/', '', specregs_clean, flags=re.S)
for mm in re.finditer(r'"([^"]+)"\s*,\s*(-?\w+)\s*,\s*([^,\n]+?)\s*,', specregs_clean):
    regs.append((mm.group(1), int(ev(mm.group(2)) if not mm.group(2).lstrip('-').isdigit() else int(mm.group(2), 0)), ev(mm.group(3))))
assert len(regs) == len(regenum), (len(regs), len(regenum))
# ---- emit -------------------------------------------------------------------
out = []
w = out.append
w('//! GENERATED by tools/gen_m68k_tables.py from vasm-1.7h/cpus/m68k/*.h.')
w('//! Do not edit. Row order and indices match vasm exactly.')
w('#![allow(dead_code, non_upper_case_globals)]')
w('')
w('#[derive(Clone, Copy)] pub struct Mnemonic { pub name: &\'static str, pub operand_type: [u8; 6], pub place: [u8; 6], pub opcode: [u16; 2], pub size: u16, pub available: u32 }')
w('#[derive(Clone, Copy)] pub struct OpType { pub modes: u16, pub flags: u16, pub first: i8, pub last: i8 }')
w('#[derive(Clone, Copy)] pub struct OperInsert { pub mode: u8, pub size: u8, pub pos: u8, pub flags: u8, pub func: &\'static str }')
w('')
for n, i in optype_idx.items():
    w(f'pub const {n}: u8 = {i};')
w('')
for n, i in place_idx.items():
    w(f'pub const PL_{n}: u8 = {i};')
w('')
for k in ['m68000','m68010','m68020','m68030','m68040','m68060','m68881','m68851','cpu32','mcfa','mcfaplus','mcfb','mcfc','mcfhwdiv','mcfmac','mcfemac','mcfusp','mcffpu','mcfmmu','mgas','malias','mfpu','m68040up','m68030up','m68020up','m68010up','m68000up','m68k','mcf','mcf_all','mfloat','mmmu','CPUMASK']:
    w(f'pub const {k}: u32 = {ev(k):#x};')
w('')
for k in ['SIZE_UNSIZED','SIZE_BYTE','SIZE_WORD','SIZE_LONG','SIZE_SINGLE','SIZE_DOUBLE','SIZE_EXTENDED','SIZE_PACKED','SIZE_MASK','SIZE_UNAMBIG','S_CFCHECK','S_NONE','S_STD','S_STD1','S_HI','S_CAS','S_MOVE','S_WL8','S_LW7','S_WL6','S_TRAP','S_EXT','S_FP','S_MAC']:
    w(f'pub const {k}: u16 = {ev(k):#x};')
w('')
for k in ['OTF_NOSIZE','OTF_BRANCH','OTF_DATA','OTF_FLTIMM','OTF_QUADIMM','OTF_SPECREG','OTF_SRRANGE','OTF_REGLIST','OTF_CHKVAL','OTF_CHKREG']:
    w(f'pub const {k}: u16 = {ev(k):#x};')
for k in ['M_nop','M_noea','M_ea','M_high_ea','M_bfea','M_kfea','M_func','M_branch','M_val0','M_reg','IIF_MASK','IIF_BCC','IIF_REVERSE','IIF_NOMODE','IIF_SIGNED','IIF_3Q','IIF_ABSVAL']:
    w(f'pub const {k}: u8 = {ev(k):#x};')
w('')
w('pub static ADDRMODES: [(i8, i8); 16] = [(0,-1),(1,-1),(2,-1),(3,-1),(4,-1),(5,-1),(6,-1),(7,0),(7,1),(7,2),(7,3),(7,4),(7,5),(7,6),(8,-1),(9,-1)];')
w('')
w(f'pub static OPTYPES: [OpType; {len(optypes)}] = [')
for (modes, flags, f, l) in optypes:
    w(f'    OpType {{ modes: {modes:#06x}, flags: {flags:#x}, first: {f}, last: {l} }},')
w('];')
w('')
w(f'pub static INSERT_INFO: [OperInsert; {len(inserts)}] = [')
for (mode, size, p, flags, func) in inserts:
    w(f'    OperInsert {{ mode: {mode}, size: {size:#x}, pos: {p}, flags: {flags:#x}, func: "{func if func != "0" else ""}" }},')
w('];')
w('')
w(f'pub static MNEMONICS: [Mnemonic; {len(mn)}] = [')
for (name, ops, places, opc, size, avail) in mn:
    ops6 = ops + [0]*(6-len(ops)); pl6 = places + [0]*(6-len(places))
    w(f'    Mnemonic {{ name: "{name}", operand_type: {ops6}, place: {pl6}, opcode: [{opc[0]:#06x}, {opc[1]:#06x}], size: {size:#06x}, available: {avail:#010x} }},')
w('];')
w('')
w(f'pub static SPECREGS: [(&str, i32, u32); {len(regs)}] = [')
for (n, c, a) in regs:
    w(f'    ("{n}", {c}, {a:#x}),')
w('];')
open(os.path.join(os.path.dirname(__file__), '..', 'src', 'm68k', 'tables.rs'), 'w').write('\n'.join(out) + '\n')
print(f'mnemonics={len(mn)} optypes={len(optypes)} inserts={len(inserts)} specregs={len(regs)}')
