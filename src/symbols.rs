//! Symbol table, transliterated from vasm 1.7h symbol.c / symtab.c.
use crate::asm::Assembler;
use crate::errors::Arg;
use crate::expr::Expr;
use crate::types::Taddr;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymKind {
    LabSym,
    Import,
    Expression,
}

pub const EXPORT: u32 = 1 << 3;
pub const INEVAL: u32 = 1 << 4;
pub const COMMON: u32 = 1 << 5;
pub const WEAK: u32 = 1 << 6;
pub const LOCAL: u32 = 1 << 7;
pub const VASMINTERN: u32 = 1 << 8;
pub const PROTECTED: u32 = 1 << 9;
pub const REFERENCED: u32 = 1 << 10;
pub const ABSLABEL: u32 = 1 << 11;
pub const EQUATE: u32 = 1 << 12;
pub const REGLIST: u32 = 1 << 13;
pub const USED: u32 = 1 << 14;
/// vasm_m only: an orphaned copy made by new_labsym() on redefinition. vasm
/// never links that copy into its symbol list, so list walkers must skip it.
pub const UNLISTED: u32 = 1 << 30;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymKind,
    pub flags: u32,
    pub expr: Option<Rc<Expr>>,
    pub size: Option<Expr>,
    pub sec: Option<usize>,
    pub pc: Taddr,
    pub align: Taddr,
    /// bumped whenever pc changes during resolve (instruction-size memo key)
    pub version: u32,
}

impl Symbol {
    /// LOCREF(s): LABSYM and not WEAK
    #[inline]
    pub fn is_locref(&self) -> bool {
        self.kind == SymKind::LabSym && self.flags & WEAK == 0
    }
    /// EXTREF(s): IMPORT or WEAK
    #[inline]
    pub fn is_extref(&self) -> bool {
        self.kind == SymKind::Import || self.flags & WEAK != 0
    }
}

#[derive(Default)]
pub struct SymTab {
    pub syms: Vec<Symbol>,
    pub map: HashMap<Vec<u8>, usize>,
    pub nocase: bool,
    pub last_global_label: String,
    pub tmplabcnt: u64,
}

impl SymTab {
    fn key(&self, name: &[u8]) -> Vec<u8> {
        if self.nocase { name.to_ascii_lowercase() } else { name.to_vec() }
    }
    /// find by str name
    pub fn find(&self, name: &str) -> Option<usize> {
        self.find_bytes(name.as_bytes())
    }
    /// find by raw bytes without allocating (unless -nocase)
    pub fn find_bytes(&self, name: &[u8]) -> Option<usize> {
        if self.nocase {
            self.map.get(&name.to_ascii_lowercase()).copied()
        } else {
            self.map.get(name).copied()
        }
    }
    fn add(&mut self, sym: Symbol) -> usize {
        let k = self.key(sym.name.as_bytes());
        let idx = self.syms.len();
        self.syms.push(sym);
        self.map.insert(k, idx);
        idx
    }
    /// refer_symbol(): make refname an alias of sym
    pub fn refer(&mut self, sym: usize, refname: &str) {
        let k = self.key(refname.as_bytes());
        self.map.insert(k, sym);
    }
}

/// is_local_label(): local names start with a blank
pub fn is_local_label(name: &str) -> bool {
    name.starts_with(' ')
}

/// make_local_label(): " " + global + " " + local
pub fn make_local_label(glob: &str, loc: &str) -> String {
    let mut s = String::with_capacity(glob.len() + loc.len() + 2);
    s.push(' ');
    s.push_str(glob);
    s.push(' ');
    s.push_str(loc);
    s
}

impl Assembler {
    pub fn find_symbol(&self, name: &str) -> Option<usize> {
        self.symtab.find(name)
    }

    /// new_import()
    pub fn new_import(&mut self, name: &str) -> usize {
        if let Some(i) = self.symtab.find(name) {
            return i;
        }
        self.symtab.add(Symbol {
            name: name.to_string(),
            kind: SymKind::Import,
            flags: 0,
            expr: None,
            size: None,
            sec: None,
            pc: 0,
            align: 0,
            version: 0,
        })
    }

    /// check_symbol(): error 67 when already defined (and not IMPORT)
    pub fn check_symbol(&mut self, name: &str) -> bool {
        if let Some(i) = self.symtab.find(name) {
            if self.symtab.syms[i].kind != SymKind::Import {
                self.general_error(67, &[Arg::from(name)]);
                return true;
            }
        }
        false
    }

    /// new_abs()
    pub fn new_abs(&mut self, name: &str, tree: Expr) -> usize {
        if let Some(i) = self.symtab.find(name) {
            let (flags, kind) = (self.symtab.syms[i].flags, self.symtab.syms[i].kind);
            if flags & EQUATE != 0 {
                self.general_error(67, &[Arg::from(name)]);
            }
            if kind != SymKind::Import && kind != SymKind::Expression {
                self.general_error(5, &[Arg::from(name)]);
            }
            let s = &mut self.symtab.syms[i];
            s.kind = SymKind::Expression;
            s.sec = None;
            s.expr = Some(Rc::new(tree));
            s.version = s.version.wrapping_add(1);
            i
        } else {
            self.symtab.add(Symbol {
                name: name.to_string(),
                kind: SymKind::Expression,
                flags: 0,
                expr: Some(Rc::new(tree)),
                size: None,
                sec: None,
                pc: 0,
                align: 0,
                version: 0,
            })
        }
    }

    /// new_equate(): non-redefinable absolute symbol
    pub fn new_equate(&mut self, name: &str, tree: Expr) -> usize {
        self.check_symbol(name);
        let i = self.new_abs(name, tree);
        self.symtab.syms[i].flags |= EQUATE;
        i
    }

    /// internal_abs()
    pub fn internal_abs(&mut self, name: &str) -> usize {
        if let Some(i) = self.symtab.find(name) {
            let s = &self.symtab.syms[i];
            if s.kind != SymKind::Expression || s.flags & (EXPORT | COMMON | WEAK) != 0 {
                self.general_error(37, &[Arg::from(name)]);
            }
            i
        } else {
            let i = self.new_abs(name, Expr::Num(0));
            self.symtab.syms[i].flags |= VASMINTERN;
            i
        }
    }

    /// set_internal_abs()
    pub fn set_internal_abs(&mut self, name: &str, val: Taddr) -> usize {
        let i = self.internal_abs(name);
        self.symtab.syms[i].expr = Some(Rc::new(Expr::Num(val)));
        self.symtab.syms[i].version = self.symtab.syms[i].version.wrapping_add(1);
        i
    }

    /// new_labsym(): define a label at the current pc of `sec` (or the current
    /// section, creating the default section if needed).
    pub fn new_labsym(&mut self, sec: Option<usize>, name: &str) -> usize {
        let sec = match sec.or(self.current_section) {
            Some(s) => s,
            None => match self.default_section() {
                Some(s) => s,
                None => {
                    self.general_error(3, &[]);
                    return self.new_import(name);
                }
            },
        };
        self.sections[sec].flags |= crate::atoms::HAS_SYMBOLS;
        let pc = self.sections[sec].pc;
        let absolute = self.sections[sec].flags & crate::atoms::ABSOLUTE != 0;
        let idx = if let Some(i) = self.symtab.find(name) {
            if self.symtab.syms[i].kind != SymKind::Import {
                // vasm warns and works on an orphaned copy; the original keeps
                // its address. We emulate by creating an unregistered symbol.
                self.general_error(5, &[Arg::from(name)]);
                let mut copy = self.symtab.syms[i].clone();
                copy.flags |= UNLISTED;
                copy.kind = SymKind::LabSym;
                copy.sec = Some(sec);
                copy.pc = pc;
                if absolute { copy.flags |= ABSLABEL } else { copy.flags &= !ABSLABEL }
                let j = self.symtab.syms.len();
                self.symtab.syms.push(copy);
                if !name.starts_with(' ') {
                    self.symtab.last_global_label = name.to_string();
                }
                return j;
            }
            let s = &mut self.symtab.syms[i];
            s.kind = SymKind::LabSym;
            s.sec = Some(sec);
            s.pc = pc;
            s.version = s.version.wrapping_add(1);
            i
        } else {
            self.symtab.add(Symbol {
                name: name.to_string(),
                kind: SymKind::LabSym,
                flags: 0,
                expr: None,
                size: None,
                sec: Some(sec),
                pc,
                align: 0,
                version: 0,
            })
        };
        if !name.starts_with(' ') {
            self.symtab.last_global_label = name.to_string();
        }
        let s = &mut self.symtab.syms[idx];
        if absolute { s.flags |= ABSLABEL } else { s.flags &= !ABSLABEL }
        idx
    }

    /// new_tmplabel()
    pub fn new_tmplabel(&mut self, sec: Option<usize>) -> usize {
        let name = format!(" *tmp{:09}*", self.symtab.tmplabcnt);
        self.symtab.tmplabcnt += 1;
        self.new_labsym(sec, &name)
    }

    /// curpc symbol: " *current pc dummy*" LABSYM, created on demand
    pub fn curpc_sym(&mut self) -> usize {
        if let Some(i) = self.cpc {
            return i;
        }
        let i = self.new_import(" *current pc dummy*");
        let s = &mut self.symtab.syms[i];
        s.kind = SymKind::LabSym;
        s.flags |= VASMINTERN | PROTECTED;
        self.cpc = Some(i);
        i
    }

    /// update_curpc()
    #[inline]
    pub fn update_curpc(&mut self, sym: usize, sec: Option<usize>, pc: Taddr) {
        if Some(sym) == self.cpc {
            if let Some(sec) = sec {
                let absolute = self.sections[sec].flags & crate::atoms::ABSOLUTE != 0;
                let s = &mut self.symtab.syms[sym];
                s.sec = Some(sec);
                s.pc = pc;
                if absolute { s.flags |= ABSLABEL } else { s.flags &= !ABSLABEL }
            }
        }
    }

    /// get_local_label() from the mot syntax module. Returns (name, new pos).
    pub fn get_local_label(&mut self, start: usize) -> Option<(String, usize)> {
        let buf = &self.line;
        let dot = self.opts.local_dots;
        let local_char = if self.opts.local_u { b'_' } else { b'.' };
        let skip_local = |p: usize| -> Option<usize> {
            if crate::chars::is_id_start(buf[p]) || crate::chars::is_digit(buf[p]) {
                let mut q = p + 1;
                while crate::chars::is_id_char(buf[q], dot) {
                    q += 1;
                }
                Some(crate::chars::chk_id_end(buf, p, q, dot))
            } else {
                None
            }
        };
        let mut s = start;
        let mut p = skip_local(s);
        let mut glob: Option<(usize, usize)> = None;
        if let Some(pp) = p {
            if buf[pp] == b'\\' && crate::chars::is_id_start(buf[s]) && buf[s] != local_char && buf[pp - 1] != b'$' {
                glob = Some((s, pp));
                s = pp + 1;
                p = skip_local(s);
            }
        }
        if let Some(pp) = p {
            if pp > s + 1 {
                let g = match glob {
                    Some((a, b)) => crate::chars::bytes_to_string(&buf[a..b]),
                    None => self.symtab.last_global_label.clone(),
                };
                if buf[s] == local_char {
                    let name = make_local_label(&g, &crate::chars::bytes_to_string(&buf[s..pp]));
                    return Some((name, crate::chars::skip(buf, pp)));
                } else if buf[pp - 1] == b'$' {
                    let name = make_local_label(&g, &crate::chars::bytes_to_string(&buf[s..pp - 1]));
                    return Some((name, crate::chars::skip(buf, pp)));
                }
            }
        }
        None
    }

    /// parse_identifier(): returns (name, new pos)
    pub fn parse_identifier(&self, p: usize) -> Option<(String, usize)> {
        let e = crate::chars::skip_identifier(&self.line, p, self.opts.local_dots)?;
        Some((crate::chars::bytes_to_string(&self.line[p..e]), e))
    }

    /// parse_symbol(): local label first, then identifier
    pub fn parse_symbol(&mut self, p: usize) -> Option<(String, usize)> {
        if let Some(r) = self.get_local_label(p) {
            return Some(r);
        }
        self.parse_identifier(p)
    }
}
