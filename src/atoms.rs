//! Atoms and sections, transliterated from vasm 1.7h atom.c / atom.h / vasm.c.
use crate::asm::Assembler;
use crate::errors::Arg;
use crate::expr::Expr;
use crate::m68k::{CpuOpt, Instruction, Operand};
use crate::types::*;

pub const HAS_SYMBOLS: u32 = 1;
pub const RESOLVE_WARN: u32 = 2;
pub const UNALLOCATED: u32 = 4;
pub const LABELS_ARE_LOCAL: u32 = 8;
pub const ABSOLUTE: u32 = 16;
pub const PREVABS: u32 = 32;
pub const IN_RORG: u32 = 64;

pub const MAXSIZECHANGES: u32 = 5;
pub const MAXPADBYTES: usize = 8;

/// Standard relocation types (reloc.h). Only the ones the m68k backend emits.
pub const REL_NONE: i32 = 0;
pub const REL_ABS: i32 = 1;
pub const REL_PC: i32 = 2;
pub const REL_SD: i32 = 10;

/// nreloc + rlist (reloc.h). vasm prepends new relocs to an atom's list, so
/// consumers that must match its order iterate this Vec in reverse.
#[derive(Debug, Clone)]
pub struct Reloc {
    pub kind: i32,
    pub byteoffset: usize,
    pub bitoffset: usize,
    pub size: usize,
    pub mask: Taddr,
    pub addend: Taddr,
    pub sym: usize,
}

#[derive(Debug, Clone)]
pub struct DBlock {
    pub data: Vec<u8>,
    pub relocs: Vec<Reloc>,
}

impl DBlock {
    #[inline]
    pub fn new(data: Vec<u8>) -> DBlock { DBlock { data, relocs: Vec::new() } }
}

#[derive(Debug, Clone)]
pub struct SBlock {
    pub space: Utaddr,
    pub space_exp: Expr,
    pub size: usize,
    pub fill: [u8; MAXPADBYTES],
    pub fill_exp: Option<Expr>,
    pub maxalignbytes: Taddr,
    pub relocs: Vec<Reloc>,
}

#[derive(Debug, Clone)]
pub struct DataDef {
    pub bitsize: i32,
    pub op: Operand,
}

#[derive(Debug, Clone)]
pub struct Assertion {
    pub assert_exp: Option<Expr>,
    pub expstr: String,
    pub msgstr: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AtomKind {
    Label(usize),
    Data(DBlock),
    Instruction(Box<Instruction>),
    Space(Box<SBlock>),
    DataDef(Box<DataDef>),
    Line(i32),
    Opts(CpuOpt),
    PrintText(String),
    PrintExpr(Box<(Expr, i32, i32)>), // expr, type, size
    Roffs(Box<Expr>),
    Rorg(Taddr),
    RorgEnd,
    Assert(Box<Assertion>),
}

#[derive(Debug, Clone)]
pub struct Atom {
    pub kind: AtomKind,
    pub align: Taddr,
    pub lastsize: usize,
    pub changes: u32,
    pub src: Option<usize>,
    pub line: i32,
}

impl Atom {
    fn mk(kind: AtomKind, align: Taddr) -> Atom {
        Atom { kind, align, lastsize: 0, changes: 0, src: None, line: 0 }
    }
    pub fn label(sym: usize) -> Atom { Atom::mk(AtomKind::Label(sym), 1) }
    pub fn data(db: DBlock, align: Taddr) -> Atom { Atom::mk(AtomKind::Data(db), align) }
    pub fn inst(ip: Instruction) -> Atom { Atom::mk(AtomKind::Instruction(Box::new(ip)), INST_ALIGN) }
    pub fn space(space_exp: Expr, size: usize, fill: Option<Expr>) -> Atom {
        Atom::mk(AtomKind::Space(Box::new(SBlock { space: 0, space_exp, size, fill: [0; MAXPADBYTES], fill_exp: fill, maxalignbytes: 0, relocs: Vec::new() })), 1)
    }
    pub fn datadef(bitsize: i32, op: Operand) -> Atom {
        Atom::mk(AtomKind::DataDef(Box::new(DataDef { bitsize, op })), data_align(bitsize))
    }
    pub fn srcline(l: i32) -> Atom { Atom::mk(AtomKind::Line(l), 1) }
    pub fn opts(o: CpuOpt) -> Atom { Atom::mk(AtomKind::Opts(o), 1) }
    pub fn text(s: String) -> Atom { Atom::mk(AtomKind::PrintText(s), 1) }
    pub fn printexpr(e: Expr, ty: i32, size: i32) -> Atom { Atom::mk(AtomKind::PrintExpr(Box::new((e, ty, size))), 1) }
    pub fn roffs(e: Expr) -> Atom { Atom::mk(AtomKind::Roffs(Box::new(e)), 1) }
    pub fn rorg(v: Taddr) -> Atom { Atom::mk(AtomKind::Rorg(v), 1) }
    pub fn rorgend() -> Atom { Atom::mk(AtomKind::RorgEnd, 1) }
    pub fn assert(a: Assertion) -> Atom { Atom::mk(AtomKind::Assert(Box::new(a)), 1) }
    #[inline]
    pub fn is_inst(&self) -> bool { matches!(self.kind, AtomKind::Instruction(_)) }
}

#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub attr: String,
    pub atoms: Vec<Atom>,
    pub align: Taddr,
    pub pad: Vec<u8>,
    pub flags: u32,
    pub memattr: u32,
    pub org: Taddr,
    pub pc: Taddr,
}

impl Section {
    pub fn has_attr(&self, c: char) -> bool { self.attr.contains(c) }
}

/// balign()
#[inline]
pub fn balign(addr: Taddr, a: Taddr) -> Taddr {
    if a != 0 { ((addr.wrapping_add(a - 1)) & !(a - 1)).wrapping_sub(addr) } else { 0 }
}

/// pcalign()
#[inline]
pub fn pcalign(a: &Atom, pc: Taddr) -> Taddr {
    let mut n = balign(pc, a.align);
    if let AtomKind::Space(sb) = &a.kind {
        if sb.maxalignbytes != 0 && n > sb.maxalignbytes { n = 0; }
    }
    pc.wrapping_add(n)
}

/// copy_cpu_taddr(): big-endian low-order bytes
pub fn copy_cpu_taddr(dest: &mut [u8], val: Taddr, bytes: usize) {
    let mut v = val;
    for i in (0..bytes).rev() {
        dest[i] = v as u8;
        v >>= 8;
    }
}

/// setval(1,...): big-endian write of `size` bytes
pub fn setval_be(dest: &mut [u8], size: usize, val: Taddr) {
    let mut v = val as u32;
    for i in (0..size).rev() {
        dest[i] = v as u8;
        v >>= 8;
    }
}

impl Assembler {
    // ----- sections ----------------------------------------------------------
    pub fn find_section(&self, name: &str, attr: &str) -> Option<usize> {
        self.sections.iter().position(|s| s.name == name && s.attr == attr)
    }

    /// new_section(): find by name+attr or create; does not switch.
    pub fn new_section(&mut self, name: &str, attr: &str, align: Taddr) -> usize {
        if let Some(i) = self.find_section(name, attr) {
            return i;
        }
        self.sections.push(Section {
            name: name.to_string(),
            attr: attr.to_string(),
            atoms: Vec::new(),
            align,
            pad: vec![0u8],
            flags: 0,
            memattr: 0,
            org: 0,
            pc: 0,
        });
        self.sections.len() - 1
    }

    /// new_org()
    pub fn new_org(&mut self, org: Taddr) -> usize {
        let name = format!("seg{:x}", org as u32);
        let s = self.new_section(&name, "acrwx", 1);
        self.sections[s].org = org;
        self.sections[s].pc = org;
        self.sections[s].flags |= ABSOLUTE;
        s
    }

    /// set_section()
    pub fn set_section(&mut self, s: usize) {
        if self.sections[s].flags & UNALLOCATED == 0 {
            self.cpu_opts_init(s);
        }
        self.current_section = Some(s);
    }

    /// switch_section()
    pub fn switch_section(&mut self, name: &str, attr: &str) {
        match self.find_section(name, attr) {
            Some(s) => self.set_section(s),
            None => self.general_error(2, &[Arg::from(name)]),
        }
    }

    /// switch_offset_section()
    pub fn switch_offset_section(&mut self, name: Option<&str>, offs: Taddr) {
        let n = match name {
            Some(n) => n.to_string(),
            None => {
                if offs != -1 { self.offset_id += 1; }
                format!("OFFSET{:06}", self.offset_id)
            }
        };
        let s = self.new_section(&n, "u", 1);
        self.sections[s].flags |= UNALLOCATED;
        if offs != -1 {
            self.sections[s].org = offs;
            self.sections[s].pc = offs;
        }
        self.set_section(s);
    }

    /// default_section(): current section, or create the syntax module's default
    pub fn default_section(&mut self) -> Option<usize> {
        if self.current_section.is_some() {
            return self.current_section;
        }
        let s = self.new_section("CODE", "acrx", 1);
        self.switch_section("CODE", "acrx");
        Some(s)
    }

    /// start_rorg()
    pub fn start_rorg(&mut self, rorg: Taddr) {
        let s = match self.default_section() {
            Some(s) => s,
            None => { self.general_error(3, &[]); return; }
        };
        if self.sections[s].flags & IN_RORG != 0 {
            self.end_rorg();
        }
        self.add_atom_to(Some(s), Atom::rorg(rorg));
        let f = &mut self.sections[s].flags;
        *f |= IN_RORG;
        if *f & ABSOLUTE == 0 {
            *f &= !PREVABS;
            *f |= ABSOLUTE;
        } else {
            *f |= PREVABS;
        }
    }

    /// end_rorg()
    pub fn end_rorg(&mut self) -> bool {
        let s = match self.default_section() {
            Some(s) => s,
            None => { self.general_error(3, &[]); return false; }
        };
        if self.sections[s].flags & IN_RORG != 0 {
            self.add_atom_to(Some(s), Atom::rorgend());
            let f = &mut self.sections[s].flags;
            if *f & PREVABS != 0 { *f |= ABSOLUTE } else { *f &= !ABSOLUTE }
            *f &= !IN_RORG;
            return true;
        }
        self.general_error(44, &[]);
        false
    }

    pub fn try_end_rorg(&mut self) {
        if let Some(cs) = self.current_section {
            if self.sections[cs].flags & IN_RORG != 0 {
                self.end_rorg();
            }
        }
    }

    // ----- atoms -------------------------------------------------------------
    /// add_atom(0, a)
    pub fn add_atom(&mut self, a: Atom) {
        self.add_atom_to(None, a)
    }

    /// Append constant bytes to the current line's align-1 DATA atom, or start
    /// a new one. Equivalent to one DATADEF atom per operand for output purposes.
    pub fn add_data_merged(&mut self, bytes: &[u8]) {
        let sec = match self.default_section() {
            Some(s) => s,
            None => { self.general_error(3, &[]); return; }
        };
        let line = self.cur_src.map(|s| self.sources[s].line).unwrap_or(0);
        let src = self.cur_src;
        let s = &mut self.sections[sec];
        // -linedebug (hunk) emits one line entry per DATA atom, so keep vasm's
        // one-atom-per-operand structure in that mode.
        if let Some(last) = s.atoms.last_mut().filter(|_| !self.opts.hunk_linedebug) {
            if last.align == 1 && last.line == line && last.src == src {
                if let AtomKind::Data(db) = &mut last.kind {
                    db.data.extend_from_slice(bytes);
                    last.lastsize += bytes.len();
                    s.pc = s.pc.wrapping_add(bytes.len() as Taddr);
                    return;
                }
            }
        }
        self.add_atom_to(Some(sec), Atom::data(DBlock::new(bytes.to_vec()), 1));
    }

    /// add_extnreloc() (reloc.c): no relocation for ORG-section labels; marks
    /// the symbol REFERENCED. Returns whether a reloc was added.
    pub fn add_extnreloc(&mut self, relocs: &mut Vec<Reloc>, sym: usize, addend: Taddr, kind: i32, bitoffs: usize, size: usize, byteoffs: usize) -> bool {
        let s = &mut self.symtab.syms[sym];
        if s.flags & crate::symbols::ABSLABEL != 0 {
            return false;
        }
        s.flags |= crate::symbols::REFERENCED;
        relocs.push(Reloc { kind, byteoffset: byteoffs, bitoffset: bitoffs, size, mask: -1, addend, sym });
        true
    }

    /// add_atom(sec, a)
    pub fn add_atom_to(&mut self, sec: Option<usize>, mut a: Atom) {
        let sec = match sec.or_else(|| self.default_section()) {
            Some(s) => s,
            None => { self.general_error(3, &[]); return; }
        };
        a.changes = 0;
        a.src = self.cur_src;
        a.line = self.cur_src.map(|s| self.sources[s].line).unwrap_or(0);
        {
            let s = &mut self.sections[sec];
            if let Some(pa) = s.atoms.last_mut() {
                if matches!(pa.kind, AtomKind::Label(_)) && pa.line == a.line
                    && matches!(a.kind, AtomKind::Instruction(_) | AtomKind::DataDef(_) | AtomKind::Space(_))
                {
                    pa.align = a.align;
                }
            }
        }
        let pc = pcalign(&a, self.sections[sec].pc);
        self.sections[sec].pc = pc;
        let size = self.atom_size(&mut a, sec, pc);
        a.lastsize = size;
        let s = &mut self.sections[sec];
        s.pc = pc.wrapping_add(size as Taddr);
        if a.align > s.align {
            s.align = a.align;
        }
        s.atoms.push(a);
    }

    /// instruction_size() with a per-instruction memo. The size is a pure
    /// function of: the referenced symbols' values, the pc, the instruction's
    /// own last_size/flags/code/qual state and the section's RESOLVE_WARN flag.
    fn instruction_size_memo(&mut self, ip: &mut crate::m68k::Instruction, sec: usize, pc: Taddr) -> usize {
        let resolvewarn = self.sections[sec].flags & RESOLVE_WARN != 0;
        if let Some(m) = &ip.memo {
            if m.pc == pc && m.last_size_in == ip.ext.last_size && m.flags_in == ip.ext.flags
                && m.code_in == ip.code && m.qual_in == ip.qual && m.resolvewarn == resolvewarn
                && m.deps.iter().all(|&(s, v)| self.symtab.syms[s as usize].version == v)
            {
                ip.ext.last_size = m.last_size_out;
                self.memo_hits += 1;
                return m.size;
            }
        }
        self.memo_misses += 1;
        let (code_in, qual_in, flags_in, last_in) = (ip.code, ip.qual, ip.ext.flags, ip.ext.last_size);
        let mut memo = match ip.memo.take() {
            Some(mut m) => { m.deps.clear(); m }
            None => Box::new(crate::m68k::InstMemo {
                deps: Vec::new(), pc: 0, last_size_in: 0, flags_in: 0, code_in: 0, qual_in: 0,
                resolvewarn: false, size: 0, last_size_out: 0,
            }),
        };
        for o in ip.op.iter().flatten() {
            for v in o.value.iter().flatten() {
                self.collect_deps(v, &mut memo.deps);
            }
        }
        let size = self.instruction_size(ip, sec, pc);
        // key = the pre-call state (what vasm's instruction_size() saw)
        memo.pc = pc;
        memo.last_size_in = last_in;
        memo.flags_in = flags_in;
        memo.code_in = code_in;
        memo.qual_in = qual_in;
        memo.resolvewarn = resolvewarn;
        memo.size = size;
        memo.last_size_out = ip.ext.last_size;
        ip.memo = Some(memo);
        size
    }

    /// Collect (symbol, version) pairs referenced by an expression, following
    /// EXPRESSION symbols. The current-pc dummy is covered by the pc key.
    fn collect_deps(&self, e: &Expr, out: &mut Vec<(u32, u32)>) {
        match e {
            Expr::Sym(s) => {
                let s = *s;
                if Some(s) == self.cpc {
                    return;
                }
                let sym = &self.symtab.syms[s];
                if out.iter().any(|&(x, _)| x as usize == s) {
                    return;
                }
                out.push((s as u32, sym.version));
                if sym.kind == crate::symbols::SymKind::Expression {
                    if let Some(x) = sym.expr.as_deref() {
                        self.collect_deps(x, out);
                    }
                }
            }
            Expr::Un(_, l) | Expr::Chain(l, _) => self.collect_deps(l, out),
            Expr::Bin(_, l, r) => {
                self.collect_deps(l, out);
                self.collect_deps(r, out);
            }
            _ => {}
        }
    }

    /// atom_size()
    pub fn atom_size(&mut self, a: &mut Atom, sec: usize, pc: Taddr) -> usize {
        match &mut a.kind {
            AtomKind::Data(db) => db.data.len(),
            AtomKind::Instruction(ip) => {
                if ip.code < 0 {
                    0
                } else if self.final_pass {
                    self.instruction_size(ip, sec, pc)
                } else {
                    self.instruction_size_memo(ip, sec, pc)
                }
            }
            AtomKind::Space(sb) => self.space_size(sb, sec, pc),
            AtomKind::DataDef(dd) => ((dd.bitsize + 7) / 8) as usize,
            AtomKind::Roffs(e) => {
                let e = (**e).clone();
                let (offs, _) = self.eval_expr(&e, Some(sec), pc);
                let offs = self.sections[sec].org.wrapping_add(offs).wrapping_sub(pc);
                if offs > 0 { offs as usize } else { 0 }
            }
            _ => 0,
        }
    }

    /// space_size()
    fn space_size(&mut self, sb: &mut SBlock, sec: usize, pc: Taddr) -> usize {
        let (space, cnst) = self.eval_expr(&sb.space_exp, Some(sec), pc);
        let space = space as Utaddr;
        if cnst || !self.final_pass {
            sb.space = space;
            if (pc as Utaddr).wrapping_add(space) < pc as Utaddr {
                self.general_error(45, &[]);
            }
        } else {
            self.general_error(30, &[]);
        }
        if self.final_pass {
            if let Some(fe) = sb.fill_exp.clone() {
                if sb.size <= BYTES_PER_TADDR {
                    let (fill, cnst) = self.eval_expr(&fe, Some(sec), pc);
                    let mut base: Option<usize> = None;
                    if !cnst {
                        let (b, bs) = self.find_base(&fe, Some(sec), pc);
                        if b == crate::expr::Base::Illegal {
                            self.general_error(38, &[]);
                        }
                        base = bs;
                    }
                    copy_cpu_taddr(&mut sb.fill, fill, sb.size);
                    if let Some(b) = base.filter(|_| sb.relocs.is_empty()) {
                        // space filled with a relocatable expression
                        let (size, space) = (sb.size, sb.space as usize);
                        let mut relocs = std::mem::take(&mut sb.relocs);
                        for i in 0..space {
                            self.add_extnreloc(&mut relocs, b, fill, REL_ABS, 0, size << 3, size * i);
                        }
                        sb.relocs = relocs;
                    }
                } else {
                    self.general_error(30, &[]);
                }
            }
        }
        sb.size * sb.space as usize
    }
}
