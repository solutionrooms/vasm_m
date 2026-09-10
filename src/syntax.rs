//! Motorola syntax module, transliterated from vasm 1.7h syntax/mot/syntax.c.
use crate::asm::{Assembler, INLSTACKSIZE, MAXCONDLEV};
use crate::atoms::{Atom, DBlock, Assertion, ABSOLUTE, IN_RORG};
use crate::chars::*;
use crate::errors::Arg;
use crate::expr::{Expr, Op};
use crate::source::{ENDM_DIRLIST, ENDR_DIRLIST, EREM_DIRLIST, REPT_DIRLIST};
use crate::symbols::{SymKind, COMMON, EXPORT, WEAK};
use crate::types::*;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const CODE_NAME: &str = "CODE";
pub const CODE_TYPE: &str = "acrx";
pub const DATA_NAME: &str = "DATA";
pub const DATA_TYPE: &str = "adrw";
pub const BSS_NAME: &str = "BSS";
pub const BSS_TYPE: &str = "aurw";
pub const RS_NAME: &str = "__RS";
pub const SO_NAME: &str = "__SO";
pub const FO_NAME: &str = "__FO";
pub const LINE_NAME: &str = "__LINE__";
pub const OPSZ_FLOAT: i32 = 0x100;
#[inline]
fn opsz_bits(size: i32) -> i32 { size & 0xff }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Org, Rorg, Section, Offset, Csec, Dsec, Bss, CodeC, CodeF, DataC, DataF, BssC, BssF,
    Global, Weak, Comm, DummyExpr, Eol, Cnop, Align, Even, Odd,
    D8, D16, D32, D64, F32, F64, F96,
    Spc8, Spc16, Spc32, Spc64, Spc96, Blk8, Blk16, Blk32, Blk64, Blk96,
    Reldata8, Reldata16, Reldata32,
    End, Fail, Idnt, List, NoList, Plen, DummyCexpr, Page, NoPage, Output, Dsource, Debug, Comment,
    IncDir, Include, IncBin, Rept, Endr, Macro, Endm, Mexit, Rem, Erem,
    IfB, IfNB, IfC, IfNC, IfD, IfND, IfMacroD, IfMacroND, IfEq, IfNe, IfGt, IfGe, IfLt, IfLe,
    Else, EndIf, RsReset, RsSet, ClrFo, SetFo,
    Rs8, Rs16, Rs32, Rs64, Rs96, Fo8, Fo16, Fo32, Fo64, Fo96,
    Cargs, PrintT, PrintV, Noop, Inline, EInline,
}

const D: u8 = 1;
const P: u8 = 2;

/// directives[] table, same order as syntax.c
pub static DIRECTIVES: &[(&str, u8, Dir)] = &[
    ("org", P | D, Dir::Org), ("rorg", P | D, Dir::Rorg), ("section", P | D, Dir::Section),
    ("offset", P | D, Dir::Offset), ("code", P | D, Dir::Csec), ("cseg", P, Dir::Csec),
    ("text", P | D, Dir::Csec), ("data", P | D, Dir::Dsec), ("dseg", P, Dir::Dsec),
    ("bss", P | D, Dir::Bss), ("code_c", P | D, Dir::CodeC), ("code_f", P | D, Dir::CodeF),
    ("data_c", P | D, Dir::DataC), ("data_f", P | D, Dir::DataF), ("bss_c", P | D, Dir::BssC),
    ("bss_f", P | D, Dir::BssF), ("public", P | D, Dir::Global), ("xdef", P | D, Dir::Global),
    ("xref", P | D, Dir::Global), ("xref.l", P | D, Dir::Global), ("nref", P, Dir::Global),
    ("entry", P, Dir::Global), ("extrn", P, Dir::Global), ("global", P, Dir::Global),
    ("import", 0, Dir::Global), ("export", 0, Dir::Global), ("weak", 0, Dir::Weak),
    ("comm", 0, Dir::Comm), ("load", P, Dir::DummyExpr), ("jumperr", 0, Dir::DummyExpr),
    ("jumpptr", 0, Dir::DummyExpr), ("mask2", 0, Dir::Eol), ("cnop", P | D, Dir::Cnop),
    ("align", 0, Dir::Align), ("even", P | D, Dir::Even), ("odd", 0, Dir::Odd),
    ("dc", P | D, Dir::D16), ("dc.b", P | D, Dir::D8), ("dc.w", P | D, Dir::D16),
    ("dc.l", P | D, Dir::D32), ("dc.q", P, Dir::D64), ("dc.s", P | D, Dir::F32),
    ("dc.d", P | D, Dir::F64), ("dc.x", P | D, Dir::F96), ("ds", P | D, Dir::Spc16),
    ("ds.b", P | D, Dir::Spc8), ("ds.w", P | D, Dir::Spc16), ("ds.l", P | D, Dir::Spc32),
    ("ds.q", P, Dir::Spc64), ("ds.s", P | D, Dir::Spc32), ("ds.d", P | D, Dir::Spc64),
    ("ds.x", P | D, Dir::Spc96), ("dcb", P | D, Dir::Blk16), ("dcb.b", P | D, Dir::Blk8),
    ("dcb.w", P | D, Dir::Blk16), ("dcb.l", P | D, Dir::Blk32), ("dcb.q", P, Dir::Blk64),
    ("dcb.s", P | D, Dir::Blk32), ("dcb.d", P | D, Dir::Blk64), ("dcb.x", P | D, Dir::Blk96),
    ("blk", P, Dir::Blk16), ("blk.b", P, Dir::Blk8), ("blk.w", P, Dir::Blk16),
    ("blk.l", P, Dir::Blk32), ("blk.q", P, Dir::Blk64), ("blk.s", P, Dir::Blk32),
    ("blk.d", P, Dir::Blk64), ("blk.x", P, Dir::Blk96), ("dr", 0, Dir::Reldata16),
    ("dr.b", 0, Dir::Reldata8), ("dr.w", 0, Dir::Reldata16), ("dr.l", 0, Dir::Reldata32),
    ("end", P | D, Dir::End), ("fail", P | D, Dir::Fail), ("idnt", P | D, Dir::Idnt),
    ("ttl", P | D, Dir::Idnt), ("module", P | D, Dir::Idnt), ("list", P | D, Dir::List),
    ("nolist", P | D, Dir::NoList), ("plen", P | D, Dir::Plen), ("llen", P | D, Dir::DummyCexpr),
    ("page", P | D, Dir::Page), ("nopage", P | D, Dir::NoPage), ("spc", P | D, Dir::DummyCexpr),
    ("output", P | D, Dir::Output), ("symdebug", P, Dir::Eol), ("dsource", P, Dir::Dsource),
    ("debug", P, Dir::Debug), ("comment", P | D, Dir::Comment), ("incdir", P | D, Dir::IncDir),
    ("include", P | D, Dir::Include), ("incbin", P | D, Dir::IncBin), ("image", 0, Dir::IncBin),
    ("rept", P | D, Dir::Rept), ("endr", P | D, Dir::Endr), ("macro", P | D, Dir::Macro),
    ("endm", P | D, Dir::Endm), ("mexit", P | D, Dir::Mexit), ("rem", P, Dir::Rem),
    ("erem", P, Dir::Erem), ("ifb", 0, Dir::IfB), ("ifnb", 0, Dir::IfNB), ("ifc", P | D, Dir::IfC),
    ("ifnc", P | D, Dir::IfNC), ("ifd", P | D, Dir::IfD), ("ifnd", P | D, Dir::IfND),
    ("ifmacrod", 0, Dir::IfMacroD), ("ifmacrond", 0, Dir::IfMacroND), ("ifeq", P | D, Dir::IfEq),
    ("ifne", P | D, Dir::IfNe), ("ifgt", P | D, Dir::IfGt), ("ifge", P | D, Dir::IfGe),
    ("iflt", P | D, Dir::IfLt), ("ifle", P | D, Dir::IfLe), ("ifmi", 0, Dir::IfLt),
    ("ifpl", 0, Dir::IfGe), ("if", P, Dir::IfNe), ("else", P | D, Dir::Else),
    ("elseif", P | D, Dir::Else), ("endif", P | D, Dir::EndIf), ("endc", P | D, Dir::EndIf),
    ("rsreset", P | D, Dir::RsReset), ("rsset", P | D, Dir::RsSet), ("clrso", P, Dir::RsReset),
    ("setso", P, Dir::RsSet), ("clrfo", P, Dir::ClrFo), ("setfo", P, Dir::SetFo),
    ("rs", P | D, Dir::Rs16), ("rs.b", P | D, Dir::Rs8), ("rs.w", P | D, Dir::Rs16),
    ("rs.l", P | D, Dir::Rs32), ("rs.q", P, Dir::Rs64), ("rs.s", P | D, Dir::Rs32),
    ("rs.d", P | D, Dir::Rs64), ("rs.x", P | D, Dir::Rs96), ("so", P, Dir::Rs16),
    ("so.b", P, Dir::Rs8), ("so.w", P, Dir::Rs16), ("so.l", P, Dir::Rs32), ("so.q", P, Dir::Rs64),
    ("so.s", P, Dir::Rs32), ("so.d", P, Dir::Rs64), ("so.x", P, Dir::Rs96), ("fo", P, Dir::Fo16),
    ("fo.b", P, Dir::Fo8), ("fo.w", P, Dir::Fo16), ("fo.l", P, Dir::Fo32), ("fo.q", P, Dir::Fo64),
    ("fo.s", P, Dir::Fo32), ("fo.d", P, Dir::Fo64), ("fo.x", P, Dir::Fo96),
    ("cargs", P | D, Dir::Cargs), ("echo", P, Dir::PrintT), ("printt", 0, Dir::PrintT),
    ("printv", 0, Dir::PrintV), ("auto", 0, Dir::Noop), ("inline", P, Dir::Inline),
    ("einline", P, Dir::EInline),
];

fn dirmap() -> &'static HashMap<&'static str, usize> {
    static M: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = HashMap::new();
        for (i, (n, _, _)) in DIRECTIVES.iter().enumerate() {
            m.entry(*n).or_insert(i);
        }
        m
    })
}

pub fn directive_exists(lname: &str) -> bool {
    dirmap().contains_key(lname)
}

impl Assembler {
    /// init_syntax()
    pub fn init_syntax(&mut self) {
        self.set_internal_abs(crate::source::REPTNSYM, -1);
        let rs = self.internal_abs(RS_NAME);
        self.symtab.refer(rs, SO_NAME);
        self.internal_abs(FO_NAME);
        self.set_internal_abs(LINE_NAME, 0);
    }

    // ----- helpers -------------------------------------------------------------
    /// eol(): check for end of line, warning 6 otherwise
    pub fn eol(&mut self, s: usize) {
        if self.opts.allow_spaces {
            let s = skip(&self.line, s);
            if !is_eol(&self.line, s) {
                self.syntax_error(6, &[]);
            }
        } else if !is_eol(&self.line, s) && !is_space(self.line[s]) {
            self.syntax_error(6, &[]);
        }
    }

    /// skip_operand()
    pub fn skip_operand(&mut self, mut s: usize) -> usize {
        let mut par_cnt = 0;
        loop {
            s = self.exp_skip(s);
            let c = self.line[s];
            if c == b'(' {
                par_cnt += 1;
            } else if c == b')' {
                if par_cnt > 0 {
                    par_cnt -= 1;
                } else {
                    self.syntax_error(3, &[]);
                }
            } else if c == b'\'' || c == b'"' {
                s = self.skip_string(s, c) - 1;
            } else if c == 0 || (par_cnt == 0 && (c == b',' || is_comment(&self.line, s))) {
                break;
            }
            s += 1;
        }
        if par_cnt != 0 {
            self.syntax_error(4, &[]);
        }
        s
    }

    /// check_directive(): returns (index, pos after name)
    fn check_directive(&self, line: usize) -> Option<(usize, usize)> {
        let s = skip(&self.line, line);
        if !is_id_start(self.line[s]) {
            return None;
        }
        let mut e = s + 1;
        while is_id_char(self.line[e], self.opts.local_dots) || self.line[e] == b'.' {
            e += 1;
        }
        let name = lower_string(&self.line[s..e]);
        dirmap().get(name.as_str()).map(|&i| (i, e))
    }

    /// parse_labeldef(): returns (name, pos after label/colon)
    fn parse_labeldef(&mut self, line: usize) -> Option<(String, usize)> {
        let mut s = line;
        let mut needcolon = false;
        if is_space(self.line[s]) {
            s = skip(&self.line, s);
            needcolon = true;
        }
        if let Some((name, ns)) = self.parse_symbol(s) {
            let mut s = skip(&self.line, ns);
            if self.line[s] == b':' {
                s += 1;
                needcolon = false;
            }
            if needcolon {
                return None;
            }
            return Some((name, s));
        }
        None
    }

    fn offs_directive(&self, s: usize, name: &[u8]) -> bool {
        let d = s + name.len();
        eq_nocase(&self.line, s, name)
            && ((is_space(self.line[d]) || is_eol(&self.line, d))
                || (self.line[d] == b'.' && (is_space(self.line[d + 2]) || is_eol(&self.line, d + 2))))
    }

    // ----- main parse loop ---------------------------------------------------
    /// parse()
    pub fn parse(&mut self) {
        while self.read_next_line() {
            if self.parse_end {
                continue;
            }
            let line = 1usize;
            let mut s = line;
            let rl = self.real_line();
            self.set_internal_abs(LINE_NAME, rl);

            if !self.cond.cond[self.cond.clev] {
                // skip source until ELSE or ENDIF
                if let Some((_, ns)) = self.parse_labeldef(s) {
                    s = ns;
                }
                s = skip(&self.line, s);
                if let Some((idx, _)) = self.check_directive(s) {
                    let (name, _, d) = DIRECTIVES[idx];
                    if name.starts_with("if") {
                        self.cond.ifnesting += 1;
                    } else if d == Dir::Else {
                        self.cond_else();
                    } else if d == Dir::EndIf {
                        self.cond_endif();
                    }
                }
                continue;
            }

            if let Some((labname, ns)) = self.parse_labeldef(s) {
                s = skip(&self.line, ns);
                let l = &self.line;
                if eq_nocase(l, s, b"equ") && is_space(l[s + 3]) {
                    s = skip(&self.line, s + 3);
                    let e = self.parse_expr_tmplab(&mut s);
                    self.new_equate(&labname, e);
                } else if eq_nocase(l, s, b"fequ.") && is_space(l[s + 6]) {
                    s += 5;
                    self.fequate(&labname, &mut s);
                } else if l[s] == b'=' {
                    s += 1;
                    s = skip(&self.line, s);
                    let e = self.parse_expr_tmplab(&mut s);
                    self.new_equate(&labname, e);
                } else if eq_nocase(l, s, b"set") && is_space(l[s + 3]) {
                    s = skip(&self.line, s + 3);
                    let e = self.parse_expr_tmplab(&mut s);
                    self.new_abs(&labname, e);
                } else if self.offs_directive(s, b"rs") || self.offs_directive(s, b"so") {
                    self.new_setoffset(Some(&labname), &mut s, RS_NAME, 1);
                } else if self.offs_directive(s, b"fo") {
                    self.new_setoffset(Some(&labname), &mut s, FO_NAME, -1);
                } else if eq_nocase(l, s, b"ttl") && is_space(l[s + 3]) {
                    s = skip(&self.line, s + 3);
                    self.filename = Some(labname.clone());
                } else if eq_nocase(l, s, b"macro") && (is_space(l[s + 5]) || l[s + 5] == 0) {
                    // reread original label field as macro name, no local macros
                    match self.parse_identifier(line) {
                        Some((name, _)) => self.new_macro(&name),
                        None => self.ierror(0, "syntax.rs", line!()),
                    }
                    continue;
                } else if self.parse_cpu_label(&labname, &mut s) {
                    // handled by cpu module (equr/reg/...)
                } else {
                    let sym = self.new_labsym(None, &labname);
                    self.add_atom(Atom::label(sym));
                }
            }

            // check for directives first
            s = skip(&self.line, s);
            let c = self.line[s];
            if c == 0 || c == b'*' || c == b';' {
                continue;
            }
            s = self.parse_cpu_special(s);
            if is_eol(&self.line, s) {
                continue;
            }
            if let Some((idx, ns)) = self.check_directive(s) {
                let ns = skip(&self.line, ns);
                self.handle_directive(DIRECTIVES[idx].2, ns);
                continue;
            }
            s = skip(&self.line, s);
            if is_eol(&self.line, s) {
                continue;
            }
            // read mnemonic name
            let inst = s;
            if !is_id_start(self.line[s]) {
                self.syntax_error(10, &[]);
                continue;
            }
            let (inst_len, quals, ns) = self.parse_instruction(s);
            s = ns;
            if !is_space(self.line[s]) && self.line[s] != 0 {
                self.syntax_error(2, &[]);
            }
            s = skip(&self.line, s);
            if self.execute_macro(inst, inst_len, &quals, s) {
                continue;
            }
            // read operands, terminated by comma (unless in parentheses)
            let mut ops: Vec<(usize, usize)> = Vec::new();
            if !is_eol(&self.line, s) {
                while ops.len() < MAX_OPERANDS {
                    let start = s;
                    s = self.skip_operand(s);
                    ops.push((start, s - start));
                    if self.opts.allow_spaces {
                        s = skip(&self.line, s);
                        if self.line[s] != b',' {
                            break;
                        }
                        s = skip(&self.line, s + 1);
                    } else {
                        if self.line[s] != b',' {
                            break;
                        }
                        s += 1;
                    }
                    if is_eol(&self.line, s) {
                        self.syntax_error(6, &[]);
                        break;
                    }
                }
                self.eol(s);
            }
            if let Some(mut ip) = self.new_inst(inst, inst_len, &ops) {
                ip.qual = quals.first().map(|(qs, _)| to_lower(self.line[*qs])).unwrap_or(0);
                if let Some((qs, ql)) = quals.first() {
                    // keep the full qualifier text for error 34 checks
                    let _ = (qs, ql);
                }
                self.add_atom(Atom::inst(ip));
            }
        }
        // cond_check()
        if self.cond.clev > 0 {
            let (src, line) = (self.cond.condsrc[self.cond.clev].clone(), self.cond.condline[self.cond.clev]);
            self.general_error(66, &[Arg::from(src), Arg::from(line)]);
        }
    }

    fn fequate(&mut self, labname: &str, s: &mut usize) {
        let x = to_lower(self.line[*s]);
        if x == b's' || x == b'd' || x == b'x' || x == b'p' {
            *s = skip(&self.line, *s + 1);
            let e = self.parse_expr_float(s);
            self.new_equate(labname, e);
        } else {
            self.syntax_error(1, &[]);
        }
    }

    // ----- conditionals (cond.c) ---------------------------------------------
    fn cond_if(&mut self, flag: bool) {
        self.cond.clev += 1;
        if self.cond.clev >= MAXCONDLEV {
            self.general_error(65, &[Arg::from(self.cond.clev)]);
            self.cond.clev = MAXCONDLEV - 1;
        }
        let cl = self.cond.clev;
        self.cond.cond[cl] = flag;
        let cs = self.cur_src.unwrap();
        self.cond.condsrc[cl] = self.sources[cs].name.clone();
        self.cond.condline[cl] = self.sources[cs].line;
    }
    fn cond_else(&mut self) {
        if self.cond.ifnesting == 0 {
            let cl = self.cond.clev;
            self.cond.cond[cl] = true;
        }
    }
    fn cond_skipelse(&mut self) {
        if self.cond.clev > 0 {
            let cl = self.cond.clev;
            self.cond.cond[cl] = false;
        } else {
            self.general_error(63, &[]);
        }
    }
    fn cond_endif(&mut self) {
        if self.cond.ifnesting == 0 {
            if self.cond.clev > 0 {
                self.cond.clev -= 1;
            } else {
                self.general_error(64, &[]);
            }
        } else {
            self.cond.ifnesting -= 1;
        }
    }

    // ----- offsets (rs/so/fo) ---------------------------------------------------
    fn new_setoffset_size(&mut self, equname: Option<&str>, symname: &str, s: &mut usize, dir: i32, size: Taddr) {
        let sym = self.internal_abs(symname);
        let old_expr = self.symtab.syms[sym].expr.as_ref().map(|e| (**e).clone()).unwrap_or(Expr::Num(0));
        let (old, new) = if !is_eol(&self.line, *s) {
            let e = self.parse_expr_tmplab(s);
            let mut new = Expr::Bin(Op::Mul, Box::new(e), Box::new(Expr::Num(size)));
            self.simplify_expr(&mut new);
            let old = if self.align_data && size > 1 {
                let dalign = (data_align((size * 8) as i32) - 1) as Taddr;
                let mut o = Expr::Bin(
                    Op::BAnd,
                    Box::new(Expr::Bin(if dir > 0 { Op::Add } else { Op::Sub }, Box::new(old_expr.clone()), Box::new(Expr::Num(dalign)))),
                    Box::new(Expr::Num(!dalign)),
                );
                self.simplify_expr(&mut o);
                o
            } else {
                old_expr.clone()
            };
            let new = Expr::Bin(if dir > 0 { Op::Add } else { Op::Sub }, Box::new(old.clone()), Box::new(new));
            (old, new)
        } else {
            (old_expr.clone(), old_expr.clone())
        };
        if let Some(eq) = equname {
            let e = if dir > 0 { old } else { new.clone() };
            self.new_equate(eq, e);
        }
        let mut new = new;
        self.simplify_expr(&mut new);
        self.symtab.syms[sym].expr = Some(std::rc::Rc::new(new));
    }

    fn new_setoffset(&mut self, equname: Option<&str>, s: &mut usize, symname: &str, dir: i32) {
        let start = *s;
        let size: Taddr;
        if self.line[start + 2] == b'.' {
            let ext = to_lower(self.line[start + 3]);
            *s = skip(&self.line, start + 4);
            size = match ext {
                b'b' => 1,
                b'w' => 2,
                b'l' | b's' => 4,
                b'q' | b'd' => 8,
                b'x' => 12,
                _ => { self.syntax_error(1, &[]); 1 }
            };
        } else {
            size = 2;
            *s = skip(&self.line, start + 2);
        }
        self.new_setoffset_size(equname, symname, s, dir, size);
    }

    // ----- data / space / alignment -------------------------------------------
    fn handle_data(&mut self, mut s: usize, size: i32) {
        loop {
            let opstart = s;
            let mut done = false;
            let c = self.line[s];
            if opsz_bits(size) == 8 && (c == b'"' || c == b'\'') {
                if let Some((data, ns)) = self.parse_string(opstart, c) {
                    self.add_atom(Atom::data(DBlock { data }, 1));
                    s = ns;
                    done = true;
                }
            }
            if !done {
                s = self.skip_operand(s);
                let ot = self.m68k_data_operand(size);
                match self.parse_operand(opstart, s - opstart, ot) {
                    Some(op) => {
                        // Fast path: a constant integer operand in range becomes
                        // bytes appended to the current line's data atom. Same
                        // bytes, same alignment (1), no relocation, no error path.
                        let bits = opsz_bits(size);
                        let merged = if !self.align_data && size == bits && matches!(bits, 8 | 16 | 32) {
                            match op.value[0].as_deref() {
                                Some(Expr::Num(v)) => {
                                    let v = *v;
                                    let ok = match bits {
                                        8 => !self.cpu.typechk || (v >= -0x80 && v <= 0xff),
                                        16 => !self.cpu.typechk || (v >= -0x8000 && v <= 0xffff),
                                        _ => true,
                                    };
                                    if ok {
                                        let n = (bits / 8) as usize;
                                        let b = (v as u32).to_be_bytes();
                                        self.add_data_merged(&b[4 - n..]);
                                        true
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        } else {
                            false
                        };
                        if !merged {
                            let mut a = Atom::datadef(bits, op);
                            if !self.align_data {
                                a.align = 1;
                            }
                            self.add_atom(a);
                        }
                    }
                    None => self.syntax_error(8, &[]),
                }
            }
            s = skip(&self.line, s);
            if self.line[s] == b',' {
                s = skip(&self.line, s + 1);
            } else {
                break;
            }
        }
    }

    fn handle_reldata(&mut self, mut s: usize, size: i32) {
        loop {
            let opstart = s;
            s = self.skip_operand(s);
            let ot = self.m68k_data_operand(size);
            match self.parse_operand(opstart, s - opstart, ot) {
                Some(mut op) => {
                    if let Some(v) = op.value[0].take() {
                        let tl = self.new_tmplabel(None);
                        self.add_atom(Atom::label(tl));
                        let mut new = Expr::Bin(Op::Sub, Box::new((*v).clone()), Box::new(Expr::Sym(tl)));
                        self.simplify_expr(&mut new);
                        op.value[0] = Some(std::rc::Rc::new(new));
                        let mut a = Atom::datadef(opsz_bits(size), op);
                        if !self.align_data {
                            a.align = 1;
                        }
                        self.add_atom(a);
                    } else {
                        self.ierror(0, "syntax.rs", line!());
                    }
                }
                None => self.syntax_error(8, &[]),
            }
            s = skip(&self.line, s);
            if self.line[s] == b',' {
                s = skip(&self.line, s + 1);
            } else {
                break;
            }
        }
    }

    fn do_space(&mut self, size: i32, cnt: Expr, fill: Option<Expr>) {
        let mut a = Atom::space(cnt, (size >> 3) as usize, fill);
        a.align = if self.align_data { data_align(size) } else { 1 };
        self.add_atom(a);
    }
    fn handle_space(&mut self, mut s: usize, size: i32) {
        let e = self.parse_expr_tmplab(&mut s);
        self.do_space(size, e, None);
    }
    fn handle_block(&mut self, mut s: usize, size: i32) {
        let cnt = self.parse_expr_tmplab(&mut s);
        s = skip(&self.line, s);
        let mut fill = None;
        if self.line[s] == b',' {
            s = skip(&self.line, s + 1);
            fill = Some(self.parse_expr_tmplab(&mut s));
        }
        self.do_space(size, cnt, fill);
    }
    fn do_alignment(&mut self, align: Taddr, offset: Expr, pad: usize, fill: Option<Expr>) {
        let mut a = Atom::space(offset, pad, fill);
        a.align = align;
        self.add_atom(a);
    }
    fn handle_cnop(&mut self, mut s: usize) {
        let offset = self.parse_expr_tmplab(&mut s);
        s = skip(&self.line, s);
        let mut align: Taddr = 1;
        if self.line[s] == b',' {
            s = skip(&self.line, s + 1);
            align = self.parse_constexpr(&mut s);
        } else {
            self.syntax_error(9, &[]);
        }
        let code_sec = match self.current_section {
            None => true,
            Some(cs) => self.sections[cs].has_attr('c'),
        };
        if !self.cpu.devpac_compat && align > 3 && code_sec {
            self.do_alignment(align, offset, 2, Some(Expr::Num(0x4e71)));
        } else {
            self.do_alignment(align, offset, 1, None);
        }
    }

    // ----- sections --------------------------------------------------------------
    fn motsection(&mut self, sec: usize, mem: u32) {
        if !self.cpu.devpac_compat {
            self.try_end_rorg();
        }
        self.sections[sec].memattr = mem;
    }
    fn nameattrsection(&mut self, name: &str, ty: &str, mem: u32) {
        let s = self.new_section(name, ty, 1);
        self.motsection(s, mem);
        self.switch_section(name, ty);
    }
    fn read_sec_attr(&mut self, s: usize) -> Option<(String, u32, usize)> {
        let ty = s;
        let e = match skip_identifier(&self.line, s, self.opts.local_dots) {
            Some(e) => e,
            None => { self.syntax_error(10, &[]); return None; }
        };
        let len = e - ty;
        let mut mem = 0u32;
        let attr = if (len == 3 || len == 5) && eq_nocase(&self.line, ty, b"bss") {
            BSS_TYPE
        } else if (len == 4 || len == 6) && eq_nocase(&self.line, ty, b"data") {
            DATA_TYPE
        } else if (len == 4 || len == 6) && (eq_nocase(&self.line, ty, b"code") || eq_nocase(&self.line, ty, b"text")) {
            CODE_TYPE
        } else {
            self.syntax_error(13, &[]);
            return None;
        };
        if len == 5 || len == 6 {
            if self.line[e - 2] == b'_' {
                match to_lower(self.line[e - 1]) {
                    b'c' => mem = 2,
                    b'f' => mem = 4,
                    b'p' => {}
                    _ => { self.syntax_error(13, &[]); return None; }
                }
            } else {
                self.syntax_error(13, &[]);
                return None;
            }
        }
        let mut s = skip(&self.line, e);
        if self.line[s] == b',' {
            s = skip(&self.line, s + 1);
            let ty = s;
            if let Some(e2) = skip_identifier(&self.line, s, self.opts.local_dots) {
                if e2 - ty == 4 && eq_nocase(&self.line, ty, b"chip") {
                    return Some((attr.to_string(), 2, skip(&self.line, e2)));
                } else if e2 - ty == 4 && eq_nocase(&self.line, ty, b"fast") {
                    return Some((attr.to_string(), 4, skip(&self.line, e2)));
                }
            }
            let mut p = ty;
            let mc = self.parse_constexpr(&mut p);
            if p > ty && mc != 0 {
                mem = mc as u32;
            } else {
                self.syntax_error(15, &[]);
            }
            s = skip(&self.line, p);
        }
        Some((attr.to_string(), mem, s))
    }
    fn handle_section(&mut self, s: usize) {
        let (name, mut s) = match self.parse_name(s) {
            Some(r) => r,
            None => return,
        };
        let attr: String;
        let mut mem = 0u32;
        if self.line[s] == b',' {
            let ns = skip(&self.line, s + 1);
            match self.read_sec_attr(ns) {
                Some((a, m, e)) => { attr = a; mem = m; s = e; }
                None => return,
            }
        } else if name.eq_ignore_ascii_case("data") {
            attr = DATA_TYPE.to_string();
        } else if name.eq_ignore_ascii_case("bss") {
            attr = BSS_TYPE.to_string();
        } else {
            attr = CODE_TYPE.to_string();
        }
        let _ = s;
        let sec = self.new_section(&name, &attr, 1);
        self.motsection(sec, mem);
        self.switch_section(&name, &attr);
    }
    fn handle_org(&mut self, s: usize) {
        if self.line[s] == b'*' {
            let s = skip(&self.line, s + 1);
            if self.line[s] == b'+' {
                let s = skip(&self.line, s + 1);
                self.handle_space(s, 8);
            } else {
                self.syntax_error(7, &[]);
            }
        } else {
            let mut s = s;
            let relocatable = match self.current_section {
                Some(cs) => self.sections[cs].flags & ABSOLUTE == 0,
                None => false,
            };
            let v = self.parse_constexpr(&mut s);
            if relocatable {
                self.start_rorg(v);
            } else {
                let sec = self.new_org(v);
                self.set_section(sec);
            }
        }
    }
    fn do_bind(&mut self, mut s: usize, bind: u32) {
        loop {
            s = skip(&self.line, s);
            let (name, ns) = match self.parse_identifier(s) {
                Some(r) => r,
                None => { self.syntax_error(10, &[]); return; }
            };
            s = ns;
            let sym = self.new_import(&name);
            let f = self.symtab.syms[sym].flags;
            if f & (EXPORT | WEAK) != 0 && f & (EXPORT | WEAK) != bind {
                let n = self.symtab.syms[sym].name.clone();
                let b = if f & EXPORT != 0 { "global" } else { "weak" };
                self.general_error(62, &[Arg::from(n), Arg::from(b)]);
            } else {
                self.symtab.syms[sym].flags |= bind;
            }
            s = skip(&self.line, s);
            let c = self.line[s];
            s += 1;
            if c != b',' {
                break;
            }
        }
    }
    fn handle_comm(&mut self, s: usize) {
        let (name, mut s) = match self.parse_identifier(s) {
            Some(r) => r,
            None => { self.syntax_error(10, &[]); return; }
        };
        let sym = self.new_import(&name);
        self.symtab.syms[sym].flags |= COMMON;
        s = skip(&self.line, s);
        let mut sz: Taddr = 4;
        if self.line[s] == b',' {
            s = skip(&self.line, s + 1);
            sz = self.parse_constexpr(&mut s);
        } else {
            self.syntax_error(9, &[]);
        }
        self.symtab.syms[sym].size = Some(Expr::Num(sz));
        self.symtab.syms[sym].align = 4;
    }

    fn ifc(&mut self, s: usize, b: bool) {
        if let Some((s1, ns)) = self.parse_name(s) {
            if self.line[ns] == b',' {
                let ns = skip(&self.line, ns + 1);
                if let Some((s2, _)) = self.parse_name(ns) {
                    self.cond_if((s1 == s2) == b);
                    return;
                }
            }
        }
        self.syntax_error(5, &[]);
    }
    fn ifdef(&mut self, s: usize, b: bool) {
        let (name, _) = match self.parse_symbol(s) {
            Some(r) => r,
            None => { self.syntax_error(10, &[]); return; }
        };
        let result = match self.find_symbol(&name) {
            Some(i) => self.symtab.syms[i].kind != SymKind::Import,
            None => false,
        };
        self.cond_if(result == b);
    }
    fn ifmacro(&mut self, s: usize, b: bool) {
        if let Some(e) = skip_identifier(&self.line, s, self.opts.local_dots) {
            let name = self.line[s..e].to_vec();
            let r = self.find_macro(&name).is_some();
            self.cond_if(r == b);
        } else {
            self.syntax_error(10, &[]);
        }
    }
    fn ifexp(&mut self, mut s: usize, c: i32) {
        let e = self.parse_expr_tmplab(&mut s);
        let (val, cnst) = self.eval_expr(&e, None, 0);
        let b = if cnst {
            match c {
                0 => val == 0,
                1 => val != 0,
                2 => val > 0,
                3 => val >= 0,
                4 => val < 0,
                _ => val <= 0,
            }
        } else {
            self.general_error(30, &[]);
            false
        };
        self.cond_if(b);
    }

    fn handle_cargs(&mut self, mut s: usize) {
        let mut offs: Expr;
        if self.line[s] == b'#' {
            s += 1;
            offs = self.parse_expr_tmplab(&mut s);
            s = skip(&self.line, s);
            if self.line[s] != b',' {
                self.syntax_error(9, &[]);
            } else {
                s = skip(&self.line, s + 1);
            }
        } else {
            offs = Expr::Num(4);
        }
        loop {
            let (name, ns) = match self.parse_symbol(s) {
                Some(r) => r,
                None => { self.syntax_error(10, &[]); break; }
            };
            s = ns;
            if !self.check_symbol(&name) {
                self.new_abs(&name, offs.clone());
            }
            let size: Taddr;
            if self.line[s] == b'.' {
                s += 1;
                match to_lower(self.line[s]) {
                    b'b' | b'w' => { size = 2; s += 1; }
                    b'l' => { size = 4; s += 1; }
                    _ => { size = 2; self.syntax_error(1, &[]); }
                }
            } else {
                size = 2;
            }
            s = skip(&self.line, s);
            if self.line[s] != b',' {
                break;
            }
            offs = Expr::Bin(Op::Add, Box::new(offs), Box::new(Expr::Num(size)));
            self.simplify_expr(&mut offs);
            s = skip(&self.line, s + 1);
        }
    }

    fn handle_printt(&mut self, mut s: usize) {
        while let Some((txt, ns)) = self.parse_name(s) {
            self.add_atom(Atom::text(txt));
            s = skip(&self.line, ns);
            if self.line[s] != b',' {
                break;
            }
            self.add_atom(Atom::text("\n".to_string()));
            s = skip(&self.line, s + 1);
        }
        self.add_atom(Atom::text("\n".to_string()));
    }
    fn handle_printv(&mut self, mut s: usize) {
        loop {
            let x = self.parse_expr(&mut s);
            self.add_atom(Atom::text("$".to_string()));
            self.add_atom(Atom::printexpr(x.clone(), 0, 32));
            self.add_atom(Atom::text(" ".to_string()));
            self.add_atom(Atom::printexpr(x.clone(), 1, 32));
            self.add_atom(Atom::text(" \"".to_string()));
            self.add_atom(Atom::printexpr(x.clone(), 3, 32));
            self.add_atom(Atom::text("\" %".to_string()));
            self.add_atom(Atom::printexpr(x, 2, 32));
            self.add_atom(Atom::text("\n".to_string()));
            s = skip(&self.line, s);
            if self.line[s] != b',' {
                break;
            }
            s = skip(&self.line, s + 1);
        }
    }

    fn handle_inline(&mut self) {
        if self.inline_stack.len() < INLSTACKSIZE {
            let id = self.inline_id;
            self.inline_id += 1;
            let name = format!("={:06}", id);
            let last = std::mem::replace(&mut self.symtab.last_global_label, name);
            if self.inline_stack.is_empty() {
                self.saved_last_global_label = Some(last);
            }
            self.inline_stack.push(id);
        } else {
            self.syntax_error(22, &[Arg::from(INLSTACKSIZE)]);
        }
    }
    fn handle_einline(&mut self) {
        if !self.inline_stack.is_empty() {
            self.inline_stack.pop();
            if self.inline_stack.is_empty() {
                self.symtab.last_global_label = self.saved_last_global_label.take().unwrap_or_default();
            } else {
                let id = *self.inline_stack.last().unwrap();
                self.symtab.last_global_label = format!("={:06}", id);
            }
        } else {
            self.syntax_error(20, &[]);
        }
    }

    // ----- dispatch -------------------------------------------------------------
    fn handle_directive(&mut self, d: Dir, s: usize) {
        match d {
            Dir::Org => self.handle_org(s),
            Dir::Rorg => {
                let mut s = s;
                let e = self.parse_expr_tmplab(&mut s);
                self.add_atom(Atom::roffs(e));
            }
            Dir::Section => self.handle_section(s),
            Dir::Offset => {
                let mut s = s;
                let offs = if !is_eol(&self.line, s) { self.parse_constexpr(&mut s) } else { -1 };
                if !self.cpu.devpac_compat {
                    self.try_end_rorg();
                }
                self.switch_offset_section(None, offs);
            }
            Dir::Csec => self.nameattrsection(CODE_NAME, CODE_TYPE, 0),
            Dir::Dsec => self.nameattrsection(DATA_NAME, DATA_TYPE, 0),
            Dir::Bss => self.nameattrsection(BSS_NAME, BSS_TYPE, 0),
            Dir::CodeC => self.nameattrsection("CODE_C", CODE_TYPE, 2),
            Dir::CodeF => self.nameattrsection("CODE_F", CODE_TYPE, 4),
            Dir::DataC => self.nameattrsection("DATA_C", DATA_TYPE, 2),
            Dir::DataF => self.nameattrsection("DATA_F", DATA_TYPE, 4),
            Dir::BssC => self.nameattrsection("BSS_C", BSS_TYPE, 2),
            Dir::BssF => self.nameattrsection("BSS_F", BSS_TYPE, 4),
            Dir::Global => self.do_bind(s, EXPORT),
            Dir::Weak => self.do_bind(s, WEAK),
            Dir::Comm => self.handle_comm(s),
            Dir::DummyExpr => {
                let mut s = s;
                let _ = self.parse_expr(&mut s);
                self.syntax_error(11, &[]);
            }
            Dir::Eol => self.eol(s),
            Dir::Cnop => self.handle_cnop(s),
            Dir::Align => {
                let mut s = s;
                let n = self.parse_constexpr(&mut s);
                self.do_alignment(1i32.wrapping_shl(n as u32), Expr::Num(0), 1, None);
            }
            Dir::Even => self.do_alignment(2, Expr::Num(0), 1, None),
            Dir::Odd => self.do_alignment(2, Expr::Num(1), 1, None),
            Dir::D8 => self.handle_data(s, 8),
            Dir::D16 => self.handle_data(s, 16),
            Dir::D32 => self.handle_data(s, 32),
            Dir::D64 => self.handle_data(s, 64),
            Dir::F32 => self.handle_data(s, OPSZ_FLOAT | 32),
            Dir::F64 => self.handle_data(s, OPSZ_FLOAT | 64),
            Dir::F96 => self.handle_data(s, OPSZ_FLOAT | 96),
            Dir::Spc8 => self.handle_space(s, 8),
            Dir::Spc16 => self.handle_space(s, 16),
            Dir::Spc32 => self.handle_space(s, 32),
            Dir::Spc64 => self.handle_space(s, 64),
            Dir::Spc96 => self.handle_space(s, 96),
            Dir::Blk8 => self.handle_block(s, 8),
            Dir::Blk16 => self.handle_block(s, 16),
            Dir::Blk32 => self.handle_block(s, 32),
            Dir::Blk64 => self.handle_block(s, 64),
            Dir::Blk96 => self.handle_block(s, 96),
            Dir::Reldata8 => self.handle_reldata(s, 8),
            Dir::Reldata16 => self.handle_reldata(s, 16),
            Dir::Reldata32 => self.handle_reldata(s, 32),
            Dir::End => self.parse_end = true,
            Dir::Fail => {
                let e = self.line[s..].iter().position(|&b| b == 0).map(|e| s + e).unwrap_or(s);
                let msg = bytes_to_string(&self.line[s..e]);
                self.add_atom(Atom::assert(Assertion { assert_exp: None, expstr: String::new(), msgstr: Some(msg) }));
            }
            Dir::Idnt => {
                if let Some((name, _)) = self.parse_name(s) {
                    self.filename = Some(name);
                }
            }
            Dir::List | Dir::NoList | Dir::Page | Dir::NoPage => {}
            Dir::Plen | Dir::DummyCexpr => {
                let mut s = s;
                let _ = self.parse_constexpr(&mut s);
                if d == Dir::DummyCexpr {
                    self.syntax_error(11, &[]);
                }
            }
            Dir::Output => {
                if let Some((name, _)) = self.parse_name(s) {
                    if name.starts_with('.') {
                        let base = self.opts.output.clone().or_else(|| self.opts.input.clone()).unwrap_or_default();
                        let stem = match base.rfind('.') {
                            Some(p) => base[..p].to_string(),
                            None => base,
                        };
                        self.opts.output = Some(format!("{}{}", stem, name));
                    } else if self.opts.output.is_none() {
                        self.opts.output = Some(name);
                    }
                }
            }
            Dir::Dsource => { let _ = self.parse_name(s); }
            Dir::Debug => {
                let mut s = s;
                let n = self.parse_constexpr(&mut s);
                self.add_atom(Atom::srcline(n));
            }
            Dir::Comment => {
                if eq_nocase(&self.line, s, b"HEAD=") {
                    let mut s = s + 5;
                    let e = self.parse_expr_tmplab(&mut s);
                    self.new_abs(" TOSFLAGS", e);
                }
            }
            Dir::IncDir => {
                let mut s = s;
                loop {
                    match self.parse_name(s) {
                        Some((name, ns)) => {
                            self.new_include_path(&name);
                            if self.line[ns] != b',' {
                                return;
                            }
                            s = skip(&self.line, ns + 1);
                        }
                        None => break,
                    }
                }
                self.syntax_error(5, &[]);
            }
            Dir::Include => {
                if let Some((name, _)) = self.parse_name(s) {
                    self.include_source(&name);
                }
            }
            Dir::IncBin => {
                if let Some((name, _)) = self.parse_name(s) {
                    self.include_binary_file(&name);
                }
            }
            Dir::Rept => {
                let mut s = s;
                let n = self.parse_constexpr(&mut s);
                self.new_repeat(n, Some(REPT_DIRLIST), ENDR_DIRLIST);
            }
            Dir::Endr => self.syntax_error(12, &[Arg::from("endr"), Arg::from("rept")]),
            Dir::Macro => {
                if let Some((name, _)) = self.parse_name(s) {
                    self.new_macro(&name);
                }
            }
            Dir::Endm => self.syntax_error(12, &[Arg::from("endm"), Arg::from("macro")]),
            Dir::Mexit => { self.leave_macro(); }
            Dir::Rem => self.new_repeat(0, None, EREM_DIRLIST),
            Dir::Erem => self.syntax_error(12, &[Arg::from("erem"), Arg::from("rem")]),
            Dir::IfB => { let b = is_eol(&self.line, s); self.cond_if(b) }
            Dir::IfNB => { let b = !is_eol(&self.line, s); self.cond_if(b) }
            Dir::IfC => self.ifc(s, true),
            Dir::IfNC => self.ifc(s, false),
            Dir::IfD => self.ifdef(s, true),
            Dir::IfND => self.ifdef(s, false),
            Dir::IfMacroD => self.ifmacro(s, true),
            Dir::IfMacroND => self.ifmacro(s, false),
            Dir::IfEq => self.ifexp(s, 0),
            Dir::IfNe => self.ifexp(s, 1),
            Dir::IfGt => self.ifexp(s, 2),
            Dir::IfGe => self.ifexp(s, 3),
            Dir::IfLt => self.ifexp(s, 4),
            Dir::IfLe => self.ifexp(s, 5),
            Dir::Else => self.cond_skipelse(),
            Dir::EndIf => self.cond_endif(),
            Dir::RsReset => { self.new_abs(RS_NAME, Expr::Num(0)); }
            Dir::RsSet => {
                let mut s = s;
                let v = self.parse_constexpr(&mut s);
                self.new_abs(RS_NAME, Expr::Num(v));
            }
            Dir::ClrFo => { self.new_abs(FO_NAME, Expr::Num(0)); }
            Dir::SetFo => {
                let mut s = s;
                let v = self.parse_constexpr(&mut s);
                self.new_abs(FO_NAME, Expr::Num(v));
            }
            Dir::Rs8 => { let mut s = s; self.new_setoffset_size(None, RS_NAME, &mut s, 1, 1) }
            Dir::Rs16 => { let mut s = s; self.new_setoffset_size(None, RS_NAME, &mut s, 1, 2) }
            Dir::Rs32 => { let mut s = s; self.new_setoffset_size(None, RS_NAME, &mut s, 1, 4) }
            Dir::Rs64 => { let mut s = s; self.new_setoffset_size(None, RS_NAME, &mut s, 1, 8) }
            Dir::Rs96 => { let mut s = s; self.new_setoffset_size(None, RS_NAME, &mut s, 1, 12) }
            Dir::Fo8 => { let mut s = s; self.new_setoffset_size(None, FO_NAME, &mut s, -1, 1) }
            Dir::Fo16 => { let mut s = s; self.new_setoffset_size(None, FO_NAME, &mut s, -1, 2) }
            Dir::Fo32 => { let mut s = s; self.new_setoffset_size(None, FO_NAME, &mut s, -1, 4) }
            Dir::Fo64 => { let mut s = s; self.new_setoffset_size(None, FO_NAME, &mut s, -1, 8) }
            Dir::Fo96 => { let mut s = s; self.new_setoffset_size(None, FO_NAME, &mut s, -1, 12) }
            Dir::Cargs => self.handle_cargs(s),
            Dir::PrintT => self.handle_printt(s),
            Dir::PrintV => self.handle_printv(s),
            Dir::Noop => self.syntax_error(11, &[]),
            Dir::Inline => self.handle_inline(),
            Dir::EInline => self.handle_einline(),
        }
        let _ = IN_RORG;
    }
}
