//! Pass driver: resolve_section(), resolve(), assemble(), fix_labels(),
//! transliterated from vasm 1.7h vasm.c.
use crate::asm::Assembler;
use crate::atoms::*;
use crate::errors::Arg;
use crate::expr::{Base, Expr};
use crate::symbols::{SymKind, ABSLABEL, COMMON, EXPORT, REFERENCED, WEAK};
use crate::types::Taddr;

pub const MAXPASSES: i32 = 1000;
pub const FASTOPTPHASE: i32 = 50;

impl Assembler {
    /// resolve_section()
    fn resolve_section(&mut self, sec: usize) {
        let mut fastphase = FASTOPTPHASE;
        let mut pass = 0;
        // Precompute runs of constant-size data atoms (DATA/DATADEF, align 1):
        // runs[i] = number of atoms in the run starting at i (0 if not a run start),
        // run_bytes[i] = their total size.
        let (runs, run_bytes) = {
            let atoms = &self.sections[sec].atoms;
            let n = atoms.len();
            let mut runs = vec![0usize; n];
            let mut run_bytes = vec![0usize; n];
            let fixed = |a: &Atom| a.align == 1 && matches!(a.kind, AtomKind::Data(_) | AtomKind::DataDef(_));
            let mut i = 0;
            while i < n {
                if fixed(&atoms[i]) {
                    let mut j = i;
                    let mut bytes = 0usize;
                    while j < n && fixed(&atoms[j]) {
                        bytes += match &atoms[j].kind {
                            AtomKind::Data(db) => db.data.len(),
                            AtomKind::DataDef(dd) => ((dd.bitsize + 7) / 8) as usize,
                            _ => 0,
                        };
                        j += 1;
                    }
                    runs[i] = j - i;
                    run_bytes[i] = bytes;
                    i = j;
                } else {
                    i += 1;
                }
            }
            (runs, run_bytes)
        };
        loop {
            let mut done = true;
            let mut rorg_pc: Taddr = 0;
            let mut org_pc: Taddr = 0;
            pass += 1;
            if pass >= MAXPASSES {
                let n = self.sections[sec].name.clone();
                self.general_error(7, &[Arg::from(n)]);
                break;
            }
            let mut extrapass = pass <= fastphase;
            self.sections[sec].pc = self.sections[sec].org;
            let mut atoms = std::mem::take(&mut self.sections[sec].atoms);
            let n = atoms.len();
            let mut i = 0;
            while i < n {
                // Fast path: a run of fixed-size, unaligned data atoms contributes
                // a constant number of bytes and has no side effects on a pass.
                if runs[i] > 1 {
                    let s = &mut self.sections[sec];
                    s.pc = s.pc.wrapping_add(run_bytes[i] as Taddr);
                    i += runs[i];
                    continue;
                }
                let a = &mut atoms[i];
                i += 1;
                let pc = pcalign(a, self.sections[sec].pc);
                self.sections[sec].pc = pc;
                self.cur_src = a.src;
                if let Some(cs) = a.src {
                    self.sources[cs].line = a.line;
                }
                match &a.kind {
                    AtomKind::Opts(o) => {
                        let o = *o;
                        self.cpu_opts(o);
                    }
                    AtomKind::Rorg(v) => {
                        if rorg_pc != 0 {
                            self.general_error(43, &[]);
                        }
                        rorg_pc = *v;
                        org_pc = pc;
                        self.sections[sec].pc = rorg_pc;
                        self.sections[sec].flags |= ABSOLUTE;
                    }
                    AtomKind::RorgEnd if rorg_pc != 0 => {
                        let s = &mut self.sections[sec];
                        s.pc = org_pc.wrapping_add(s.pc.wrapping_sub(rorg_pc));
                        rorg_pc = 0;
                        s.flags &= !ABSOLUTE;
                    }
                    AtomKind::Label(l) => {
                        let l = *l;
                        if self.symtab.syms[l].kind != SymKind::LabSym {
                            self.ierror(0, "resolve.rs", line!());
                        }
                        let pc = self.sections[sec].pc;
                        if self.symtab.syms[l].pc != pc {
                            done = false;
                            self.symtab.syms[l].pc = pc;
                            self.symtab.syms[l].version = self.symtab.syms[l].version.wrapping_add(1);
                        }
                    }
                    _ => {}
                }
                if pass > fastphase && !done && a.is_inst() {
                    // safe mode: optimize only one instruction per pass
                    let s = &mut self.sections[sec];
                    s.pc = s.pc.wrapping_add(a.lastsize as Taddr);
                    continue;
                }
                let pc = self.sections[sec].pc;
                let size = if a.changes > MAXSIZECHANGES {
                    self.sections[sec].flags |= RESOLVE_WARN;
                    let sz = self.atom_size(a, sec, pc);
                    self.sections[sec].flags &= !RESOLVE_WARN;
                    sz
                } else {
                    self.atom_size(a, sec, pc)
                };
                if size != a.lastsize {
                    done = false;
                    if pass > fastphase {
                        a.changes += 1;
                    } else if size > a.lastsize {
                        extrapass = false;
                    }
                    a.lastsize = size;
                }
                let s = &mut self.sections[sec];
                s.pc = s.pc.wrapping_add(size as Taddr);
            }
            self.sections[sec].atoms = atoms;
            if rorg_pc != 0 {
                let s = &mut self.sections[sec];
                s.pc = org_pc.wrapping_add(s.pc.wrapping_sub(rorg_pc));
                s.flags &= !ABSOLUTE;
            }
            if extrapass {
                fastphase += 1;
            }
            if !(self.errs.errors == 0 && !done) {
                if std::env::var("VASM_M_TIMING").is_ok() {
                    eprintln!("resolve: section {} took {} passes ({} atoms)", self.sections[sec].name, pass, self.sections[sec].atoms.len());
                }
                break;
            }
        }
    }

    /// resolve()
    pub fn resolve(&mut self) {
        self.final_pass = false;
        for sec in 0..self.sections.len() {
            self.resolve_section(sec);
        }
    }

    /// convert_offset_labels()
    fn convert_offset_labels(&mut self) {
        for i in 0..self.symtab.syms.len() {
            let s = &self.symtab.syms[i];
            if s.kind == SymKind::LabSym {
                if let Some(sec) = s.sec {
                    if self.sections[sec].flags & UNALLOCATED != 0 {
                        let pc = s.pc;
                        let s = &mut self.symtab.syms[i];
                        s.kind = SymKind::Expression;
                        s.sec = None;
                        s.expr = Some(std::rc::Rc::new(Expr::Num(pc)));
                    }
                }
            }
        }
    }

    /// assemble(): final pass
    pub fn assemble(&mut self) {
        self.convert_offset_labels();
        self.final_pass = true;
        for sec in 0..self.sections.len() {
            let mut lasterr: Option<(Option<usize>, i32)> = None;
            self.sections[sec].pc = self.sections[sec].org;
            let bss = self.sections[sec].has_attr('u');
            let mut rorg_pc: Taddr = 0;
            let mut org_pc: Taddr = 0;
            let n = self.sections[sec].atoms.len();
            for i in 0..n {
                let mut a = std::mem::replace(&mut self.sections[sec].atoms[i], Atom::rorgend());
                let basepc = self.sections[sec].pc;
                let pc = pcalign(&a, basepc);
                self.sections[sec].pc = pc;
                self.cur_src = a.src;
                if let Some(cs) = a.src {
                    self.sources[cs].line = a.line;
                }
                if a.changes > MAXSIZECHANGES {
                    self.sections[sec].flags |= RESOLVE_WARN;
                }
                if pc != basepc {
                    // warn on auto-aligned instructions or data
                    let next_same_line = matches!(a.kind, AtomKind::Label(_))
                        && i + 1 < n
                        && self.sections[sec].atoms[i + 1].line == a.line;
                    let is_inst;
                    let is_data;
                    if next_same_line {
                        let k = &self.sections[sec].atoms[i + 1].kind;
                        is_inst = matches!(k, AtomKind::Instruction(_));
                        is_data = matches!(k, AtomKind::Data(_) | AtomKind::DataDef(_));
                    } else {
                        is_inst = matches!(a.kind, AtomKind::Instruction(_));
                        is_data = matches!(a.kind, AtomKind::Data(_) | AtomKind::DataDef(_));
                    }
                    if is_inst {
                        self.general_error(50, &[]);
                    } else if is_data {
                        self.general_error(57, &[]);
                    }
                }
                match &mut a.kind {
                    AtomKind::Rorg(v) => {
                        rorg_pc = *v;
                        org_pc = pc;
                        self.sections[sec].pc = rorg_pc;
                        self.sections[sec].flags |= ABSOLUTE;
                    }
                    AtomKind::RorgEnd => {
                        if rorg_pc != 0 {
                            let s = &mut self.sections[sec];
                            s.pc = org_pc.wrapping_add(s.pc.wrapping_sub(rorg_pc));
                            rorg_pc = 0;
                            s.flags &= !ABSOLUTE;
                        } else {
                            self.general_error(44, &[]);
                        }
                    }
                    AtomKind::Instruction(ip) => {
                        let pc = self.sections[sec].pc;
                        let db = self.eval_instruction(ip, sec, pc);
                        a.kind = AtomKind::Data(db);
                    }
                    AtomKind::DataDef(dd) => {
                        let pc = self.sections[sec].pc;
                        let db = self.eval_data(&mut dd.op, dd.bitsize, sec, pc);
                        a.kind = AtomKind::Data(db);
                    }
                    AtomKind::Roffs(e) => {
                        let e = (**e).clone();
                        let pc = self.sections[sec].pc;
                        let (space, cnst) = self.eval_expr(&e, Some(sec), pc);
                        if cnst {
                            let space = self.sections[sec].org.wrapping_add(space).wrapping_sub(pc);
                            if space >= 0 {
                                a.kind = AtomKind::Space(Box::new(SBlock {
                                    space: 0,
                                    space_exp: Expr::Num(space),
                                    size: 1,
                                    fill: [0; MAXPADBYTES],
                                    fill_exp: None,
                                    maxalignbytes: 0,
                                }));
                            } else {
                                self.general_error(20, &[]);
                            }
                        } else {
                            self.general_error(30, &[]);
                        }
                    }
                    AtomKind::Opts(o) => {
                        let o = *o;
                        self.cpu_opts(o);
                    }
                    AtomKind::PrintText(t) => print!("{}", t),
                    AtomKind::PrintExpr(pe) => {
                        let (e, ty, size) = (pe.0.clone(), pe.1, pe.2);
                        let pc = self.sections[sec].pc;
                        self.atom_printexpr(&e, ty, size, sec, pc);
                    }
                    AtomKind::Assert(ast) => {
                        let msg = ast.msgstr.clone().unwrap_or_default();
                        if let Some(e) = ast.assert_exp.clone() {
                            let pc = self.sections[sec].pc;
                            let (val, _) = self.eval_expr(&e, Some(sec), pc);
                            if val == 0 {
                                let es = ast.expstr.clone();
                                self.general_error(47, &[Arg::from(es), Arg::from(msg)]);
                            }
                        } else {
                            self.general_error(19, &[Arg::from(msg)]);
                        }
                    }
                    _ => {}
                }
                if matches!(a.kind, AtomKind::Data(_)) && bss {
                    if lasterr != Some((a.src, a.line)) {
                        if self.sections[sec].flags & UNALLOCATED != 0 {
                            if self.warn_unalloc_ini_dat {
                                self.general_error(54, &[]);
                            }
                        } else {
                            self.general_error(31, &[]);
                        }
                        lasterr = Some((a.src, a.line));
                    }
                }
                let pc = self.sections[sec].pc;
                let size = self.atom_size(&mut a, sec, pc);
                let s = &mut self.sections[sec];
                s.pc = s.pc.wrapping_add(size as Taddr);
                s.flags &= !RESOLVE_WARN;
                self.sections[sec].atoms[i] = a;
            }
            if rorg_pc != 0 {
                let s = &mut self.sections[sec];
                s.pc = org_pc.wrapping_add(s.pc.wrapping_sub(rorg_pc));
                s.flags &= !ABSOLUTE;
            }
        }
        // remove_unalloc_sects(): keep the list, but mark for the writer
    }

    fn atom_printexpr(&mut self, e: &Expr, ty: i32, size: i32, sec: usize, pc: Taddr) {
        let (val, _) = self.eval_expr(e, Some(sec), pc);
        let mask: u64 = if size >= 32 { 0xffffffff } else { (1u64 << size) - 1 };
        let u = (val as u32 as u64) & mask;
        match ty {
            0 => print!("{:x}", u),
            1 => print!("{}", val),
            2 => print!("{:b}", u),
            _ => {
                let mut s = String::new();
                let bytes = (size / 8) as usize;
                for i in (0..bytes).rev() {
                    let c = ((u >> (i * 8)) & 0xff) as u8;
                    s.push(if (32..127).contains(&c) { c as char } else { '.' });
                }
                print!("{}", s);
            }
        }
    }

    /// undef_syms()
    pub fn undef_syms(&mut self) {
        for i in 0..self.symtab.syms.len() {
            let s = &self.symtab.syms[i];
            if !self.auto_import && s.kind == SymKind::Import && s.flags & (EXPORT | COMMON | WEAK) == 0 {
                let n = s.name.clone();
                self.general_error(22, &[Arg::from(n)]);
            } else if s.kind == SymKind::Import && s.flags & REFERENCED == 0 {
                let n = s.name.clone();
                self.general_error(61, &[Arg::from(n)]);
            }
        }
    }

    /// fix_labels()
    pub fn fix_labels(&mut self) {
        for i in 0..self.symtab.syms.len() {
            let s = &self.symtab.syms[i];
            if s.flags & ABSLABEL != 0 && s.kind == SymKind::LabSym {
                let pc = s.pc;
                let s = &mut self.symtab.syms[i];
                s.kind = SymKind::Expression;
                s.flags &= !(7 | COMMON);
                s.sec = None;
                s.size = None;
                s.align = 0;
                s.expr = Some(std::rc::Rc::new(Expr::Num(pc)));
            } else if s.kind == SymKind::Expression {
                let e = s.expr.clone().unwrap_or_else(|| std::rc::Rc::new(Expr::Num(0)));
                let (val, cnst) = self.eval_expr(&e, None, 0);
                if !cnst {
                    let (b, base) = self.find_base(&e, None, 0);
                    if b == Base::Ok {
                        let base = base.unwrap();
                        let (bk, bsec) = (self.symtab.syms[base].kind, self.symtab.syms[base].sec);
                        let s = &mut self.symtab.syms[i];
                        s.kind = bk;
                        s.sec = bsec;
                        s.pc = val;
                        s.align = 1;
                    } else {
                        let n = self.symtab.syms[i].name.clone();
                        self.general_error(53, &[Arg::from(n)]);
                    }
                }
            }
        }
    }
}
