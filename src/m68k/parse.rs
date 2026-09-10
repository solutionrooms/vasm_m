//! m68k operand parsing and mnemonic selection, transliterated from
//! vasm 1.7h cpus/m68k/cpu.c (getreg, scan_Rnlist, get_any_register,
//! getbasereg, set_index, parse_operand, parse_instruction, cpu specials)
//! and atom.c new_inst().
use super::tables::*;
use super::*;
use crate::asm::{Assembler, RegSym};
use crate::atoms::Atom;
use crate::chars::*;
use crate::errors::Arg;
use crate::expr::Expr;
use crate::types::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

pub const REGAn: i32 = 8;
pub const REGPC: i32 = 16;
pub const REGZero: i32 = 0x80;
pub const REGext_Shift: i32 = 8;
pub const REGscale_Shift: i32 = 12;
#[inline] pub fn reg_get(n: i32) -> i8 { (n & 7) as i8 }
#[inline] pub fn reg_geta(n: i32) -> i32 { n & 15 }
#[inline] pub fn reg_is_an(n: i32) -> bool { n & REGAn != 0 }
#[inline] pub fn reg_is_pc(n: i32) -> bool { n & REGPC != 0 }
#[inline] pub fn reg_is_zero(n: i32) -> bool { n & REGZero != 0 }
#[inline] pub fn reg_ext(n: i32) -> i32 { (n & 0x700) >> 8 }
#[inline] pub fn reg_scale(n: i32) -> i32 { (n & 0x3000) >> 12 }

pub const PO_CORRUPT: i32 = -1;
pub const PO_NOMATCH: i32 = 0;
pub const PO_MATCH: i32 = 1;

/// OCMD_* option commands
pub const OCMD_NOP: u8 = 0;
pub const OCMD_CPU: u8 = 1;
pub const OCMD_FPU: u8 = 2;
pub const OCMD_SDREG: u8 = 3;
pub const OCMD_NOOPT: u8 = 4;
pub const OCMD_OPTGEN: u8 = 5;
pub const OCMD_OPTMOVEM: u8 = 6;
pub const OCMD_OPTPEA: u8 = 7;
pub const OCMD_OPTCLR: u8 = 8;
pub const OCMD_OPTST: u8 = 9;
pub const OCMD_OPTLSL: u8 = 10;
pub const OCMD_OPTMUL: u8 = 11;
pub const OCMD_OPTDIV: u8 = 12;
pub const OCMD_OPTFCONST: u8 = 13;
pub const OCMD_OPTBRAJMP: u8 = 14;
pub const OCMD_OPTPC: u8 = 15;
pub const OCMD_OPTBRA: u8 = 16;
pub const OCMD_OPTDISP: u8 = 17;
pub const OCMD_OPTABS: u8 = 18;
pub const OCMD_OPTMOVEQ: u8 = 19;
pub const OCMD_OPTQUICK: u8 = 20;
pub const OCMD_OPTBRANOP: u8 = 21;
pub const OCMD_OPTBDISP: u8 = 22;
pub const OCMD_OPTODISP: u8 = 23;
pub const OCMD_OPTLEA: u8 = 24;
pub const OCMD_OPTLQUICK: u8 = 25;
pub const OCMD_OPTIMMADDR: u8 = 26;
pub const OCMD_OPTSPEED: u8 = 27;
pub const OCMD_SMALLCODE: u8 = 28;
pub const OCMD_OPTWARN: u8 = 29;
pub const OCMD_CHKPIC: u8 = 30;
pub const OCMD_CHKTYPE: u8 = 31;
pub const OCMD_NOWARN: u8 = 32;

/// mnemonic hash: lowercase name -> first table index (mgas rows removed, as init_cpu does)
fn mnemo_map() -> &'static HashMap<Vec<u8>, usize> {
    static M: OnceLock<HashMap<Vec<u8>, usize>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = HashMap::new();
        let mut i = 0;
        while i < MNEMONICS.len() {
            let name = MNEMONICS[i].name;
            if !name.is_empty() {
                m.entry(name.to_ascii_lowercase().into_bytes()).or_insert(i);
            }
            let mut j = i + 1;
            while j < MNEMONICS.len() && MNEMONICS[j].name == name {
                j += 1;
            }
            i = j;
        }
        // remove gas mnemonics
        let mut i = 0;
        while i < MNEMONICS.len() {
            if MNEMONICS[i].available & mgas != 0 {
                m.remove(MNEMONICS[i].name.to_ascii_lowercase().as_bytes());
                while i + 1 < MNEMONICS.len() && MNEMONICS[i].name == MNEMONICS[i + 1].name {
                    i += 1;
                }
            }
            i += 1;
        }
        m
    })
}

pub fn mnemonic_exists(lname: &str) -> bool {
    mnemo_map().contains_key(lname.as_bytes())
}

#[inline]
pub fn getextcode(c: u8) -> i32 {
    match to_lower(c) {
        b'b' => EXT_BYTE,
        b'w' => EXT_WORD,
        b'l' => EXT_LONG,
        b's' => EXT_SINGLE,
        b'd' => EXT_DOUBLE,
        b'x' => EXT_EXTENDED,
        b'p' => EXT_PACKED,
        _ => 0,
    }
}

#[inline]
pub fn branch_size(ext: u8) -> usize {
    match ext {
        b'b' | b's' => 0,
        b'l' => 4,
        _ => 2,
    }
}

impl Assembler {
    /// m68k_data_operand()
    pub fn m68k_data_operand(&mut self, bits: i32) -> u8 {
        match bits {
            8 => OP_D8,
            16 => OP_D16,
            32 => OP_D32,
            64 => OP_D64,
            0x120 => OP_F32,
            0x140 => OP_F64,
            0x160 => OP_F96,
            _ => {
                self.cpu_error(38, &[Arg::from(bits & 0xff)]);
                0
            }
        }
    }

    /// parse_instruction(): returns (mnemonic length, qualifiers [(start,len)], new pos)
    pub fn parse_instruction(&mut self, s: usize) -> (usize, Vec<(usize, usize)>, usize) {
        let inst = s;
        let mut s = s;
        if self.line[s] == b'.' {
            s += 1;
        }
        while self.line[s] != 0 && self.line[s] != b'.' && !is_space(self.line[s]) {
            s += 1;
        }
        let inst_len = s - inst;
        let mut quals = Vec::new();
        while self.line[s] == b'.' && quals.len() < MAX_QUALIFIERS {
            s += 1;
            let es = s;
            while self.line[s] != 0 && self.line[s] != b'.' && !is_space(self.line[s]) {
                s += 1;
            }
            if s - es == 0 {
                self.cpu_error(34, &[]);
            } else {
                quals.push((es, s - es));
            }
        }
        self.cpu.current_ext = quals.first().map(|(q, _)| to_lower(self.line[*q])).unwrap_or(0);
        (inst_len, quals, s)
    }

    /// read_extension()
    fn read_extension(&self, s: &mut usize, extcode: i32) -> i32 {
        let mut p = *s;
        if self.line[p] == b'.' {
            p += 1;
            let x = getextcode(self.line[p]);
            p += 1;
            if x != 0 {
                *s = p;
                return x;
            }
        }
        extcode
    }

    fn is_float_ext(&self) -> bool {
        matches!(self.cpu.current_ext, b's' | b'd' | b'x' | b'p')
    }

    /// find_regsym()
    fn find_regsym(&self, name: &[u8]) -> Option<&RegSym> {
        self.regsyms.get(&bytes_to_string(name))
    }

    /// getreg(): returns packed register byte or -1
    pub fn getreg(&mut self, start: &mut usize, an_only: bool, indexreg: bool) -> i32 {
        let s0 = *start;
        let mut reg: i32 = -1;
        if is_id_start(self.line[s0]) || (self.cpu.elfregs && self.line[s0] == b'%') {
            let mut p = s0;
            let mut s = s0 + 1;
            while is_id_char(self.line[s], self.opts.local_dots) && self.line[s] != b'.' {
                s += 1;
            }
            if let Some(sym) = self.find_regsym(&self.line[p..s]) {
                if sym.rtype == 0 || sym.rtype == 1 {
                    reg = (if sym.rtype == 1 { 8 } else { 0 }) | sym.reg;
                }
            } else {
                if self.cpu.elfregs {
                    if self.line[p] == b'%' { p += 1; } else { return -1; }
                }
                if s - p == 2 {
                    let c0 = self.line[p];
                    let c1 = self.line[p + 1];
                    if (c0 == b'D' || c0 == b'd' || c0 == b'A' || c0 == b'a') && (b'0'..=b'7').contains(&c1) {
                        reg = (if c0 == b'A' || c0 == b'a' { 8 } else { 0 }) | (c1 - b'0') as i32;
                    } else if (c0 == b'S' || c0 == b's') && (c1 == b'P' || c1 == b'p') {
                        reg = 15;
                    }
                }
            }
            if reg >= 0 {
                if an_only && reg & 8 == 0 {
                    self.cpu_error(4, &[]);
                }
                if self.line[s] == b'.' {
                    let extcode = if !indexreg {
                        match to_lower(self.line[s + 1]) { b'l' => 3, b'u' => 2, _ => 0 }
                    } else {
                        getextcode(self.line[s + 1])
                    };
                    if extcode != 0 {
                        reg |= extcode << 4;
                        *start = s + 2;
                    }
                } else {
                    *start = s;
                }
            }
        }
        reg
    }

    /// scan_Rnlist()
    pub fn scan_rnlist(&mut self, start: &mut usize) -> u16 {
        let mut p = *start;
        if self.getreg(&mut p, false, false) < 0 {
            return 0;
        }
        let mut list: u16 = 0;
        let mut lastreg: i32 = -1;
        let mut rangemode = false;
        p = *start;
        while self.line[p] != 0 {
            p = skip(&self.line, p);
            let mut reg: i32;
            if rangemode && (b'0'..=b'7').contains(&self.line[p]) {
                reg = (lastreg & REGAn) | (self.line[p] - b'0') as i32;
                p += 1;
            } else {
                reg = self.getreg(&mut p, false, false);
                if reg < 0 {
                    self.cpu_error(2, &[]);
                    return 0;
                }
            }
            if rangemode {
                list &= !(1u16 << lastreg);
                if lastreg > reg {
                    std::mem::swap(&mut lastreg, &mut reg);
                } else if lastreg == reg {
                    self.cpu_error(17, &[]);
                }
                for rx in lastreg..=reg {
                    if list & (1u16 << rx) != 0 {
                        self.cpu_error(17, &[]);
                    } else {
                        list |= 1u16 << rx;
                    }
                }
                rangemode = false;
            } else if list & (1u16 << reg) != 0 {
                self.cpu_error(17, &[]);
            } else {
                list |= 1u16 << reg;
            }
            lastreg = reg;
            p = skip(&self.line, p);
            if self.line[p] == b'-' {
                rangemode = true;
            } else if self.line[p] != b'/' {
                break;
            }
            p += 1;
        }
        *start = p;
        list
    }

    /// get_any_register(): Dn/An/register list/special register
    fn get_any_register(&mut self, start: &mut usize, op: &mut Operand, required: u8) -> bool {
        let s = *start;
        let ot = OPTYPES[required as usize];
        let reg = self.getreg(start, false, false);
        if reg >= 0 {
            let p = skip(&self.line, *start);
            if self.line[p] == b'-' || self.line[p] == b'/' || ot.flags & OTF_REGLIST != 0 {
                op.mode = MODE_Extended;
                op.reg = REG_RnList;
                *start = s;
                let l = self.scan_rnlist(start);
                op.value[0] = Some(Rc::new(Expr::Num(l as Taddr)));
            } else {
                let sf = reg >> 4;
                op.mode = if reg_is_an(reg) { MODE_An } else { MODE_Dn };
                op.reg = reg_get(reg);
                if sf == 2 || sf == 3 || (ot.flags as u32) & FL_MAC != 0 {
                    op.flags |= FL_MAC;
                    op.bf_offset = (sf == 2) as u8;
                }
            }
            return true;
        }
        // (no FPU on 68000: getfreg never matches)
        // special registers (CCR, SR, ...)
        let mut s2 = s;
        let mut name: Option<usize> = None;
        if (self.line[s2] == b'<' && self.line[s2 + 1] == b'<') || (self.line[s2] == b'>' && self.line[s2 + 1] == b'>') {
            name = Some(s2);
            s2 += 2;
        } else {
            if self.cpu.elfregs {
                if self.line[s2] != b'%' { return false; }
                s2 += 1;
            }
            if is_id_start(self.line[s2]) {
                name = Some(s2);
                s2 += 1;
                while is_id_char(self.line[s2], self.opts.local_dots) {
                    s2 += 1;
                }
            }
        }
        if let Some(n) = name {
            let len = s2 - n;
            let (mut i, last) = if ot.flags & OTF_SRRANGE != 0 { (ot.first as usize, ot.last as usize) } else { (0, SPECREGS.len() - 1) };
            while i <= last {
                let (rname, code, avail) = SPECREGS[i];
                if rname.len() == len && eq_nocase_slice(&self.line[n..s2], rname.as_bytes()) {
                    if avail & self.cpu.cpu_type == 0 {
                        break;
                    }
                    op.mode = MODE_SpecReg;
                    op.reg = i as i8;
                    op.value[0] = Some(Rc::new(Expr::Num(code)));
                    *start = s2;
                    return true;
                }
                i += 1;
            }
        }
        false
    }

    /// getbasereg()
    fn getbasereg(&mut self, start: &mut usize) -> i32 {
        let s0 = *start;
        let mut r: i32 = 0;
        if is_id_start(self.line[s0]) || (self.cpu.elfregs && self.line[s0] == b'%') {
            let mut p = s0;
            let mut s = s0 + 1;
            while is_id_char(self.line[s], self.opts.local_dots) && self.line[s] != b'.' {
                s += 1;
            }
            if self.cpu.elfregs {
                if self.line[p] == b'%' { p += 1; } else { r = -1; }
            }
            if s - p == 3 && (self.line[p] == b'z' || self.line[p] == b'Z') {
                r |= REGZero;
                p += 1;
            }
            if s - p == 2 {
                let c0 = self.line[p];
                let c1 = self.line[p + 1];
                if eq_nocase(&self.line, p, b"PC") {
                    r |= REGPC;
                } else if (c0 == b'D' || c0 == b'd' || c0 == b'A' || c0 == b'a') && (b'0'..=b'7').contains(&c1) {
                    r |= (if c0 == b'A' || c0 == b'a' { REGAn } else { 0 }) | (c1 - b'0') as i32;
                } else {
                    r = -1;
                }
                if r >= 0 {
                    p += 2;
                    if self.line[p] == b'.' {
                        let extcode = getextcode(self.line[p + 1]);
                        r |= extcode << REGext_Shift;
                        *start = p + 2;
                    } else {
                        *start = p;
                    }
                }
            } else {
                r = -1;
            }
            if r < 0 {
                let reg = self.getreg(start, false, true);
                r = if reg >= 0 { ((reg & 0x70) << 4) | reg_geta(reg) } else { -1 };
            }
            if r >= 0 && self.line[*start] == b'*' {
                match self.line[*start + 1] {
                    b'1' => {}
                    b'2' => r |= 1 << REGscale_Shift,
                    b'4' => r |= 2 << REGscale_Shift,
                    b'8' => r |= 3 << REGscale_Shift,
                    _ => {
                        self.cpu_error(10, &[]);
                        return r;
                    }
                }
                *start += 2;
            }
            return r;
        }
        -1
    }

    /// set_index()
    fn set_index(&mut self, op: &mut Operand, i: i32) {
        if i >= 0 {
            let s = reg_scale(i) as u16;
            op.flags |= FL_UsesFormat;
            if reg_is_zero(i) {
                op.format |= FW_FullFormat | FW_IndexSuppress;
                op.flags |= FL_020up;
            } else if s != 0 {
                if self.cpu.cpu_type & mcf == 0 || (s > 2 && self.cpu.cpu_type & mcffpu == 0) {
                    op.flags |= FL_020up;
                }
            }
            op.format |= (if reg_is_an(i) { FW_IndexAn } else { 0 })
                | fw_index_reg((i & 7) as u16)
                | (if reg_ext(i) == EXT_LONG { FW_LongIndex } else { 0 })
                | fw_scale(s);
        }
    }

    /// check_basereg()
    fn check_basereg(&mut self, op: &mut Operand) {
        if op.reg >= 0 && op.reg <= 6 {
            if let (Some(be), Some(v)) = (self.baseexp[op.reg as usize].clone(), op.value[0].clone()) {
                let (b, _) = self.find_base(&v, None, 0);
                if b == crate::expr::Base::Ok {
                    let mut new = Expr::Bin(crate::expr::Op::Sub, Box::new((*v).clone()), Box::new(be));
                    self.simplify_expr(&mut new);
                    op.value[0] = Some(Rc::new(new));
                    op.flags |= FL_BaseReg;
                }
            }
        }
    }

    /// fix_basereg()
    pub fn fix_basereg(&mut self, op: &mut Operand) -> bool {
        if let Some(v) = op.value[0].clone() {
            if let Expr::Bin(_, left, _) = &*v {
                let (_, cnst) = self.eval_expr(left, None, 0);
                if cnst {
                    op.value[0] = Some(Rc::new((**left).clone()));
                    op.flags &= !FL_BaseReg;
                    return true;
                }
            }
        }
        false
    }

    fn parse_immediate(&mut self, start: &mut usize, op: &mut Operand, is_float: bool, is_quad: bool) {
        let e = if is_float {
            self.parse_expr_float(start)
        } else if is_quad {
            self.parse_expr_huge(start)
        } else {
            self.parse_expr(start)
        };
        op.value[0] = Some(Rc::new(e));
    }

    /// base_disp_and_ext(): parse expression into value[0]; returns disp ext or -1
    fn base_disp_and_ext(&mut self, op: &mut Operand, p: &mut usize) -> i32 {
        let e = self.parse_expr(p);
        op.value[0] = Some(Rc::new(e));
        let disp_size = self.read_extension(p, 0);
        if disp_size != 0 {
            op.flags |= FL_NoOptBase;
        }
        disp_size
    }

    /// parse_operand(): returns PO_MATCH/PO_NOMATCH/PO_CORRUPT and the operand
    pub fn parse_operand_rc(&mut self, start: usize, len: usize, required: u8) -> (i32, Operand) {
        let reqmode = OPTYPES[required as usize].modes;
        let reqflags = OPTYPES[required as usize].flags;
        let mut op = Operand::new();
        let mut p = skip(&self.line, start);
        if reqflags & OTF_DATA != 0 {
            op.mode = MODE_Extended;
            op.reg = REG_Immediate;
            self.parse_immediate(&mut p, &mut op, reqflags & OTF_FLTIMM != 0, reqflags & OTF_QUADIMM != 0);
        } else if self.line[p] == b'#' || (self.cpu.sgs && self.line[p] == b'&') {
            p += 1;
            op.mode = MODE_Extended;
            op.reg = REG_Immediate;
            let fl = reqflags & OTF_FLTIMM != 0 && self.is_float_ext();
            self.parse_immediate(&mut p, &mut op, fl, reqflags & OTF_QUADIMM != 0);
        } else if self.get_any_register(&mut p, &mut op, required) {
            let ptmp = skip(&self.line, p);
            if self.line[ptmp] == b':' && op.mode == MODE_Dn {
                let mut pt = skip(&self.line, ptmp + 1);
                let reg = self.getreg(&mut pt, false, false);
                if reg >= 0 && !reg_is_an(reg) {
                    op.reg |= (reg << 4) as i8;
                    op.flags |= FL_DoubleReg | FL_020up;
                    p = pt;
                } else {
                    self.cpu_error(18, &[]);
                }
            }
            p = skip(&self.line, p);
        } else {
            let mut disp_size: i32 = 0;
            let mut start_term: Option<usize> = None;
            if self.line[p] == b'-' && self.line[p + 1] == b'(' {
                let mut ptmp = skip(&self.line, p + 2);
                let reg = self.getreg(&mut ptmp, true, false);
                if reg >= 0 {
                    ptmp = skip(&self.line, ptmp);
                    if self.line[ptmp] != b')' {
                        self.cpu_error(3, &[]);
                    } else {
                        ptmp += 1;
                    }
                    p = ptmp;
                    op.mode = MODE_AnPreDec;
                    op.reg = reg_get(reg);
                }
            }
            // parse_expression:
            loop {
                if (self.line[p] != b'(' || start_term.is_some()) && op.mode < 0 {
                    disp_size = self.base_disp_and_ext(&mut op, &mut p);
                    p = skip(&self.line, p);
                }
                if self.line[p] == b'(' && op.mode < 0 {
                    start_term = Some(p);
                    p = skip(&self.line, p + 1);
                    let mut idx: i32 = -1;
                    if self.line[p] == b'[' {
                        // 020+ memory indirect: not supported on 68000
                        self.cpu_error(1, &[]);
                        return (PO_CORRUPT, op);
                    }
                    let mut reg = self.getbasereg(&mut p);
                    if reg < 0 && op.value[0].is_none() {
                        disp_size = self.base_disp_and_ext(&mut op, &mut p);
                        p = skip(&self.line, p);
                        if self.line[p] == b')' {
                            // expression was only the first term: read the full expression
                            p = start_term.unwrap();
                            continue;
                        }
                        if self.line[p] == b',' {
                            p = skip(&self.line, p + 1);
                            reg = self.getbasereg(&mut p);
                            if reg < 0 {
                                self.cpu_error(7, &[]);
                                return (PO_CORRUPT, op);
                            }
                        }
                    }
                    if op.value[0].is_some() && disp_size > EXT_LONG {
                        self.cpu_error(5, &[]);
                        disp_size = 0;
                    }
                    p = skip(&self.line, p);
                    if reg >= 0 {
                        if (reg_is_an(reg) || reg_is_pc(reg)) && reg_scale(reg) == 0 && reg_ext(reg) == 0 {
                            // base register: try to read index
                            loop {
                                if self.line[p] == b',' {
                                    p = skip(&self.line, p + 1);
                                    idx = self.getbasereg(&mut p);
                                    if idx >= 0 {
                                        p = skip(&self.line, p);
                                    } else if op.value[0].is_none() {
                                        // (An,bd) is treated as (bd,An)
                                        disp_size = self.base_disp_and_ext(&mut op, &mut p);
                                        if disp_size < 0 {
                                            self.cpu_error(12, &[]);
                                            return (PO_CORRUPT, op);
                                        }
                                        if !self.cpu.devpac_compat {
                                            self.cpu_error(6, &[]);
                                        }
                                        p = skip(&self.line, p);
                                        continue;
                                    } else {
                                        self.cpu_error(12, &[]);
                                        return (PO_CORRUPT, op);
                                    }
                                }
                                break;
                            }
                        } else {
                            // Rn is already the index, assume ZA0 as base
                            if (reqflags as u32) & FL_DoubleReg == 0 {
                                idx = reg;
                                reg = REGZero | REGAn;
                            }
                        }
                    }
                    if self.line[p] == b',' && op.value[0].is_none() {
                        p = skip(&self.line, p + 1);
                        disp_size = self.base_disp_and_ext(&mut op, &mut p);
                        if disp_size >= 0 {
                            p = skip(&self.line, p);
                            if !self.cpu.devpac_compat {
                                self.cpu_error(6, &[]);
                            }
                        }
                    }
                    if self.line[p] != b')' {
                        self.cpu_error(15, &[Arg::C(b')')]);
                    }
                    p += 1;
                    if reg >= 0 {
                        if idx >= 0 {
                            if reg_is_pc(idx) {
                                self.cpu_error(16, &[]);
                                return (PO_CORRUPT, op);
                            }
                            if reg_ext(idx) != 0 && reg_ext(idx) != EXT_LONG && reg_ext(idx) != EXT_WORD {
                                self.cpu_error(5, &[]);
                                idx &= !(7 << REGext_Shift);
                            }
                        }
                        if disp_size == 0 {
                            disp_size = if idx < 0 { EXT_WORD } else { EXT_BYTE };
                        }
                        if !reg_is_zero(reg) && op.value[1].is_none()
                            && ((idx < 0 && disp_size == EXT_WORD) || (idx >= 0 && disp_size == EXT_BYTE && !reg_is_zero(idx)))
                        {
                            if idx < 0 {
                                if op.value[0].is_some() {
                                    if reg_is_pc(reg) {
                                        op.mode = MODE_Extended;
                                        op.reg = REG_PC16Disp;
                                    } else {
                                        op.mode = MODE_An16Disp;
                                        op.reg = reg_get(reg);
                                        self.check_basereg(&mut op);
                                    }
                                } else if self.line[p] == b'+' {
                                    op.mode = MODE_AnPostInc;
                                    op.reg = reg_get(reg);
                                    p += 1;
                                } else {
                                    op.mode = MODE_AnIndir;
                                    op.reg = reg_get(reg);
                                    let ptmp = skip(&self.line, p);
                                    if self.line[ptmp] == b':' {
                                        self.cpu_error(1, &[]);
                                        return (PO_CORRUPT, op);
                                    }
                                }
                            } else {
                                if op.value[0].is_none() {
                                    op.value[0] = Some(Rc::new(Expr::Num(0)));
                                }
                                if reg_is_pc(reg) {
                                    op.mode = MODE_Extended;
                                    op.reg = REG_PC8Format;
                                } else {
                                    op.mode = MODE_An8Format;
                                    op.reg = reg_get(reg);
                                    self.check_basereg(&mut op);
                                }
                                self.set_index(&mut op, idx);
                            }
                        } else {
                            // full format word (020+): mark; encoder rejects on 68000
                            op.format |= FW_FullFormat;
                            op.flags |= FL_UsesFormat | FL_020up;
                            if reg_is_pc(reg) {
                                op.mode = MODE_Extended;
                                op.reg = REG_PC8Format;
                            } else {
                                op.mode = MODE_An8Format;
                                op.reg = reg_get(reg);
                                self.check_basereg(&mut op);
                            }
                            if reg_is_zero(reg) {
                                op.format |= FW_BaseSuppress;
                            }
                            if idx < 0 {
                                idx = REGZero;
                            }
                            self.set_index(&mut op, idx);
                            if op.value[0].is_some() {
                                op.format |= if disp_size == EXT_LONG { 0x30 } else { 0x20 };
                            } else {
                                op.format |= 0x10;
                            }
                        }
                    }
                }
                break;
            }
            if op.mode < 0 && op.value[0].is_some() {
                op.mode = MODE_Extended;
                if disp_size == EXT_WORD {
                    op.reg = REG_AbsShort;
                } else {
                    if reqflags & OTF_REGLIST != 0 {
                        op.reg = if required == RL { REG_RnList } else { REG_FPnList };
                    } else {
                        op.reg = REG_AbsLong;
                    }
                    if disp_size != 0 && disp_size != EXT_LONG {
                        self.cpu_error(5, &[]);
                    }
                }
            }
            p = skip(&self.line, p);
            if self.line[p] == b'&' || (reqflags as u32) & FL_MAC != 0 {
                op.flags |= FL_MAC;
                if self.line[p] == b'&' {
                    op.bf_width = 1;
                    p = skip(&self.line, p + 1);
                } else {
                    op.bf_width = 0;
                }
            }
        }
        if self.line[p] == b'{' {
            // bitfield / k-factor: 020+ only
            self.cpu_error(1, &[]);
            return (PO_CORRUPT, op);
        }
        // compare parsed addressing mode against requirements
        for i in 0..16 {
            if reqmode & (1 << i) != 0 {
                let (am, ar) = ADDRMODES[i];
                if (op.flags & FL_CheckMask) == ((reqflags as u32) & FL_CheckMask) && am == op.mode && (ar < 0 || ar == op.reg) {
                    if reqflags & OTF_CHKREG != 0 {
                        let ot = OPTYPES[required as usize];
                        if op.reg < ot.first || op.reg > ot.last {
                            return (PO_NOMATCH, op);
                        }
                    }
                    if required == DP {
                        op.flags |= FL_NoOpt;
                        if op.mode == MODE_AnIndir {
                            op.mode = MODE_An16Disp;
                            op.value[0] = Some(Rc::new(Expr::Num(0)));
                            self.cpu_error(48, &[Arg::from(op.reg as i32), Arg::from(op.reg as i32)]);
                        }
                    }
                    p = skip(&self.line, p);
                    if self.line[p] == 0 || p >= start + len {
                        return (PO_MATCH, op);
                    }
                }
            }
        }
        (PO_NOMATCH, op)
    }

    /// parse_operand() as used by data directives: Some(op) on match
    pub fn parse_operand(&mut self, start: usize, len: usize, required: u8) -> Option<Operand> {
        let (rc, op) = self.parse_operand_rc(start, len, required);
        if rc == PO_MATCH { Some(op) } else { None }
    }

    /// new_inst(): mnemonic selection (atom.c)
    pub fn new_inst(&mut self, inst: usize, len: usize, ops: &[(usize, usize)]) -> Option<Instruction> {
        let mut lbuf = [0u8; 32];
        let ll = len.min(32);
        for (k, b) in self.line[inst..inst + ll].iter().enumerate() {
            lbuf[k] = b.to_ascii_lowercase();
        }
        let lname = &lbuf[..ll];
        let mut inst_found = 0;
        if let Some(&first) = mnemo_map().get(lname) {
            let mut i = first;
            loop {
                inst_found = 1;
                let m = &MNEMONICS[i];
                let mnemo_opcnt = m.operand_type.iter().position(|&t| t == 0).unwrap_or(MAX_OPERANDS);
                inst_found = 2;
                self.save_symbols();
                let mut parsed: Vec<Operand> = Vec::with_capacity(mnemo_opcnt);
                let mut j = 0;
                let mut k = 0;
                let mut corrupt = false;
                while j < mnemo_opcnt {
                    if k >= ops.len() {
                        break;
                    }
                    let (rc, op) = self.parse_operand_rc(ops[k].0, ops[k].1, MNEMONICS[i].operand_type[j]);
                    if rc == PO_CORRUPT {
                        corrupt = true;
                        break;
                    }
                    if rc == PO_NOMATCH {
                        break;
                    }
                    parsed.push(op);
                    k += 1;
                    j += 1;
                }
                if corrupt {
                    self.restore_symbols();
                    return None;
                }
                if j < mnemo_opcnt || k < ops.len() {
                    i += 1;
                    self.restore_symbols();
                    if i < MNEMONICS.len() && MNEMONICS[i].name.len() == ll && MNEMONICS[i].name.as_bytes().eq_ignore_ascii_case(lname) {
                        continue;
                    }
                    break;
                }
                let mut ip = Instruction::new(i as i32);
                for (n, op) in parsed.into_iter().enumerate() {
                    ip.op[n] = Some(op);
                }
                return Some(ip);
            }
        }
        match inst_found {
            1 => self.general_error(8, &[]),
            2 => self.general_error(0, &[]),
            _ => {
                let n = bytes_to_string(&self.line[inst..inst + len]);
                self.general_error(1, &[Arg::from(n)]);
            }
        }
        None
    }

    // ----- cpu options ---------------------------------------------------------
    /// cpu_opts()
    pub fn cpu_opts(&mut self, o: CpuOpt) {
        let (cmd, arg) = (o.cmd, o.arg);
        if cmd > OCMD_NOOPT && cmd < OCMD_OPTWARN && arg != 0 {
            self.cpu.no_opt = false;
        }
        let b = arg != 0;
        let c = &mut self.cpu;
        match cmd {
            OCMD_NOP => {}
            OCMD_CPU => {
                c.cpu_type = arg as u32;
                let v = (arg as u32 & CPUMASK) as Taddr;
                self.set_internal_abs("__VASM", v);
            }
            OCMD_FPU => {}
            OCMD_SDREG => c.sdreg = arg,
            OCMD_NOOPT => c.no_opt = b,
            OCMD_OPTGEN => c.opt_gen = b,
            OCMD_OPTMOVEM => c.opt_movem = b,
            OCMD_OPTPEA => c.opt_pea = b,
            OCMD_OPTCLR => c.opt_clr = b,
            OCMD_OPTST => c.opt_st = b,
            OCMD_OPTLSL => c.opt_lsl = b,
            OCMD_OPTMUL => c.opt_mul = b,
            OCMD_OPTDIV => c.opt_div = b,
            OCMD_OPTFCONST => c.opt_fconst = b,
            OCMD_OPTBRAJMP => c.opt_brajmp = b,
            OCMD_OPTPC => c.opt_pc = b,
            OCMD_OPTBRA => c.opt_bra = b,
            OCMD_OPTDISP => c.opt_disp = b,
            OCMD_OPTABS => c.opt_abs = b,
            OCMD_OPTMOVEQ => c.opt_moveq = b,
            OCMD_OPTQUICK => c.opt_quick = b,
            OCMD_OPTBRANOP => c.opt_branop = b,
            OCMD_OPTBDISP => c.opt_bdisp = b,
            OCMD_OPTODISP => c.opt_odisp = b,
            OCMD_OPTLEA => c.opt_lea = b,
            OCMD_OPTLQUICK => c.opt_lquick = b,
            OCMD_OPTIMMADDR => c.opt_immaddr = b,
            OCMD_OPTSPEED => c.opt_speed = b,
            OCMD_SMALLCODE => c.opt_sc = b,
            OCMD_OPTWARN => c.warn_opts = arg as u8,
            OCMD_CHKPIC => {}
            OCMD_CHKTYPE => c.typechk = b,
            OCMD_NOWARN => self.errs.no_warn = b,
            _ => self.ierror(0, "parse.rs", line!()),
        }
    }

    /// add_cpu_opt()
    pub fn add_cpu_opt(&mut self, sec: Option<usize>, cmd: u8, arg: i32) {
        let o = CpuOpt { cmd, arg };
        if sec.is_some() || self.current_section.is_some() {
            self.add_atom_to(sec, Atom::opts(o));
            self.cpu_opts(o);
        } else {
            self.cpu_opts(o);
        }
    }

    fn cpu_opts_optinit(&mut self, s: Option<usize>) {
        let c = self.cpu.clone();
        let list: [(u8, bool); 25] = [
            (OCMD_NOOPT, c.no_opt), (OCMD_OPTGEN, c.opt_gen), (OCMD_OPTMOVEM, c.opt_movem),
            (OCMD_OPTPEA, c.opt_pea), (OCMD_OPTCLR, c.opt_clr), (OCMD_OPTST, c.opt_st),
            (OCMD_OPTLSL, c.opt_lsl), (OCMD_OPTMUL, c.opt_mul), (OCMD_OPTDIV, c.opt_div),
            (OCMD_OPTFCONST, c.opt_fconst), (OCMD_OPTBRAJMP, c.opt_brajmp), (OCMD_OPTPC, c.opt_pc),
            (OCMD_OPTBRA, c.opt_bra), (OCMD_OPTDISP, c.opt_disp), (OCMD_OPTABS, c.opt_abs),
            (OCMD_OPTMOVEQ, c.opt_moveq), (OCMD_OPTQUICK, c.opt_quick), (OCMD_OPTBRANOP, c.opt_branop),
            (OCMD_OPTBDISP, c.opt_bdisp), (OCMD_OPTODISP, c.opt_odisp), (OCMD_OPTLEA, c.opt_lea),
            (OCMD_OPTLQUICK, c.opt_lquick), (OCMD_OPTIMMADDR, c.opt_immaddr), (OCMD_OPTSPEED, c.opt_speed),
            (OCMD_SMALLCODE, c.opt_sc),
        ];
        for (cmd, v) in list {
            self.add_cpu_opt(s, cmd, v as i32);
        }
    }

    /// cpu_opts_init()
    pub fn cpu_opts_init(&mut self, s: usize) {
        let ct = self.cpu.cpu_type as i32;
        self.add_cpu_opt(Some(s), OCMD_CPU, ct);
        self.add_cpu_opt(Some(s), OCMD_FPU, 1);
        let sd = self.cpu.sdreg;
        self.add_cpu_opt(Some(s), OCMD_SDREG, sd);
        self.cpu_opts_optinit(Some(s));
        let w = self.cpu.warn_opts as i32;
        self.add_cpu_opt(Some(s), OCMD_OPTWARN, w);
        self.add_cpu_opt(Some(s), OCMD_CHKPIC, 0);
        let t = self.cpu.typechk as i32;
        self.add_cpu_opt(Some(s), OCMD_CHKTYPE, t);
        let nw = self.errs.no_warn as i32;
        self.add_cpu_opt(Some(s), OCMD_NOWARN, nw);
    }

    fn validchar(&self, s: usize) -> u8 {
        if is_eol(&self.line, s) { 0 } else { self.line[s] }
    }

    /// devpac_option(): returns new pos or None on error
    fn devpac_option(&mut self, mut s: usize) -> Option<usize> {
        const OPT_MAP: [u8; 12] = [
            OCMD_OPTBRA, OCMD_OPTDISP, OCMD_OPTABS, OCMD_OPTMOVEQ, OCMD_OPTQUICK, OCMD_NOP,
            OCMD_OPTBRANOP, OCMD_OPTBDISP, OCMD_OPTODISP, OCMD_OPTLEA, OCMD_OPTLQUICK, OCMD_OPTIMMADDR,
        ];
        let mut flag = 1;
        if eq_nocase(&self.line, s, b"no") {
            flag = 0;
            s += 2;
        }
        if eq_nocase(&self.line, s, b"autopc") {
            self.add_cpu_opt(None, OCMD_OPTPC, flag);
            return Some(s + 6);
        } else if eq_nocase(&self.line, s, b"case") {
            self.symtab.nocase = flag == 0;
            return Some(s + 4);
        } else if eq_nocase(&self.line, s, b"chkpc") {
            self.add_cpu_opt(None, OCMD_CHKPIC, flag);
            return Some(s + 5);
        } else if eq_nocase(&self.line, s, b"debug") {
            return Some(s + 5);
        } else if eq_nocase(&self.line, s, b"symtab") {
            return Some(s + 6);
        } else if eq_nocase(&self.line, s, b"type") {
            self.add_cpu_opt(None, OCMD_CHKTYPE, flag);
            return Some(s + 4);
        } else if eq_nocase(&self.line, s, b"warn") {
            self.errs.no_warn = flag == 0;
            self.add_cpu_opt(None, OCMD_NOWARN, (flag == 0) as i32);
            return Some(s + 4);
        } else if eq_nocase(&self.line, s, b"xdebug") {
            if flag != 0 {
                self.opts.no_symbols = false;
            }
            self.hunk_onlyglobal = flag != 0; // only xdef-symbols in objects for Amiga
            return Some(s + 6);
        } else if flag == 0 {
            self.cpu_error(23, &[]);
            return None;
        }
        if eq_nocase(&self.line, s, b"p=") {
            s += 1;
            loop {
                s += 1;
                match self.get_cpu_type(&mut s) {
                    Some(cpu) => self.set_cpu_type(cpu, true),
                    None => self.cpu_error(43, &[]),
                }
                if self.line[s] != b'/' {
                    break;
                }
            }
            return Some(s);
        }
        let opt = to_lower(self.validchar(s));
        if opt != 0 {
            let mut ext = self.validchar(s + 1);
            let mut num = 0;
            if ext.is_ascii_digit() {
                let mut e = s + 1;
                while self.line[e].is_ascii_digit() {
                    num = num * 10 + (self.line[e] - b'0') as i32;
                    e += 1;
                }
            }
            let mut c;
            loop {
                s += 1;
                c = self.validchar(s);
                if c == 0 || c == b'+' || c == b'-' {
                    break;
                }
            }
            if c != 0 {
                let flag = (c == b'+') as i32;
                if ext == c {
                    ext = 0;
                }
                match opt {
                    b'a' => self.add_cpu_opt(None, OCMD_OPTPC, flag),
                    b'c' => self.symtab.nocase = flag == 0,
                    b'd' | b'm' | b's' => {}
                    b'x' => {
                        if flag != 0 {
                            self.opts.no_symbols = false;
                        }
                        self.hunk_onlyglobal = flag != 0; // only xdef-symbols in objects for Amiga
                    }
                    b'l' => {
                        if flag == 0 {
                            self.cpu_error(23, &[]);
                            return None;
                        }
                    }
                    b'o' => {
                        if ext.is_ascii_digit() && (1..=12).contains(&num) {
                            self.add_cpu_opt(None, OPT_MAP[(num - 1) as usize], flag);
                        } else {
                            match to_lower(ext) {
                                0 => {
                                    self.add_cpu_opt(None, OCMD_NOOPT, (flag == 0) as i32);
                                    if !self.cpu.devpac_compat {
                                        self.add_cpu_opt(None, OCMD_OPTGEN, flag);
                                        self.add_cpu_opt(None, OCMD_OPTFCONST, flag);
                                        self.add_cpu_opt(None, OCMD_OPTBRAJMP, flag);
                                    }
                                    for cmd in [OCMD_OPTPC, OCMD_OPTBRA, OCMD_OPTDISP, OCMD_OPTABS, OCMD_OPTMOVEQ, OCMD_OPTQUICK, OCMD_OPTBRANOP, OCMD_OPTBDISP, OCMD_OPTODISP, OCMD_OPTLEA, OCMD_OPTLQUICK, OCMD_OPTIMMADDR] {
                                        self.add_cpu_opt(None, cmd, flag);
                                    }
                                }
                                b'c' => self.add_cpu_opt(None, OCMD_OPTCLR, flag),
                                b'd' => self.add_cpu_opt(None, OCMD_OPTDIV, flag),
                                b'f' => self.add_cpu_opt(None, OCMD_OPTFCONST, flag),
                                b'g' => self.add_cpu_opt(None, OCMD_OPTGEN, flag),
                                b'j' => self.add_cpu_opt(None, OCMD_OPTBRAJMP, flag),
                                b'l' => self.add_cpu_opt(None, OCMD_OPTLSL, flag),
                                b'm' => self.add_cpu_opt(None, OCMD_OPTMOVEM, flag),
                                b'p' => self.add_cpu_opt(None, OCMD_OPTPEA, flag),
                                b's' => self.add_cpu_opt(None, OCMD_OPTSPEED, flag),
                                b't' => self.add_cpu_opt(None, OCMD_OPTST, flag),
                                b'w' => self.add_cpu_opt(None, OCMD_OPTWARN, if flag != 0 { 2 } else { 0 }),
                                b'x' => self.add_cpu_opt(None, OCMD_OPTMUL, flag),
                                _ => self.cpu_error(31, &[Arg::C(opt), Arg::C(ext)]),
                            }
                        }
                    }
                    b'p' => self.add_cpu_opt(None, OCMD_CHKPIC, flag),
                    b't' => self.add_cpu_opt(None, OCMD_CHKTYPE, flag),
                    b'w' => {
                        self.errs.no_warn = flag == 0;
                        self.add_cpu_opt(None, OCMD_NOWARN, (flag == 0) as i32);
                    }
                    _ => self.cpu_error(31, &[Arg::C(opt.to_ascii_uppercase()), Arg::C(c)]),
                }
                return Some(s + 1);
            }
        }
        self.cpu_error(23, &[]);
        None
    }

    fn get_cpu_type(&mut self, s: &mut usize) -> Option<u32> {
        let start = *s;
        while is_id_char(self.line[*s], self.opts.local_dots) {
            *s += 1;
        }
        let name = lower_string(&self.line[start..*s]);
        let t = match name.as_str() {
            "68000" | "68008" => Some(m68000),
            "68010" => Some(m68010),
            "68020" => Some(m68020),
            "68030" => Some(m68030),
            "68040" => Some(m68040),
            "68060" => Some(m68060),
            "68020up" => Some(m68020up),
            "68881" | "68882" => Some(m68881),
            "68851" => Some(m68851),
            _ => None,
        };
        if t.is_none() && self.cur_src.is_some() {
            self.cpu_error(43, &[]);
        }
        t
    }

    fn set_cpu_type(&mut self, ty: u32, addatom: bool) {
        if ty & (m68k | cpu32 | mcf_all) != 0 {
            self.cpu.cpu_type = (self.cpu.cpu_type & !(m68k | cpu32 | mcf_all)) | ty;
        } else if ty & m68881 != 0 {
            self.cpu.cpu_type = (self.cpu.cpu_type & !m68881) | ty;
        } else if ty == m68851 {
            self.cpu.cpu_type |= m68851;
        }
        if addatom {
            let ct = self.cpu.cpu_type as i32;
            self.add_cpu_opt(None, OCMD_CPU, ct);
        }
    }

    fn skip_line(&self, mut s: usize) -> usize {
        while self.line[s] != 0 {
            s += 1;
        }
        s
    }

    /// parse_cpu_special(): cpu-specific directives; returns pos after them
    pub fn parse_cpu_special(&mut self, start: usize) -> usize {
        let name = start;
        let mut s = start;
        if !is_id_start(self.line[s]) {
            return start;
        }
        s += 1;
        while is_id_char(self.line[s], self.opts.local_dots) {
            s += 1;
        }
        let len = s - name;
        let l = &self.line;
        if (len == 4 && eq_nocase(l, name, b"near")) || (len == 6 && eq_nocase(l, name, b".sdreg")) {
            s = skip(&self.line, s);
            let r = self.getreg(&mut s, false, false);
            if r >= 0 {
                if (REGAn + 2..=REGAn + 6).contains(&r) {
                    self.cpu.sdreg = reg_get(r) as i32;
                } else {
                    self.cpu_error(58, &[]);
                }
            } else if self.line[name] != b'.' && eq_nocase(&self.line, s, b"code") {
                self.add_cpu_opt(None, OCMD_SMALLCODE, 1);
                return self.skip_line(s);
            } else {
                self.cpu.sdreg = self.last_sdreg;
            }
            let sd = self.cpu.sdreg;
            self.add_cpu_opt(None, OCMD_SDREG, sd);
            return self.skip_line(s);
        } else if len == 3 && eq_nocase(l, name, b"far") {
            if self.cpu.sdreg >= 0 {
                self.last_sdreg = self.cpu.sdreg;
                self.cpu.sdreg = -1;
                self.add_cpu_opt(None, OCMD_SDREG, -1);
            }
            self.add_cpu_opt(None, OCMD_SMALLCODE, 0);
            self.eol(s);
            return self.skip_line(s);
        } else if len == 8 && eq_nocase(l, name, b"initnear") {
            self.cpu_error(59, &[]);
            return self.skip_line(s);
        } else if len == 7 && eq_nocase(l, name, b"basereg") {
            s = skip(&self.line, s);
            let e = self.parse_expr(&mut s);
            s = skip(&self.line, s);
            if self.line[s] == b',' {
                s = skip(&self.line, s + 1);
                let reg = self.getreg(&mut s, false, false);
                if (REGAn..=REGAn + 6).contains(&reg) {
                    let r = reg_get(reg) as usize;
                    if self.baseexp[r].is_some() && !self.cpu.phxass_compat {
                        self.cpu_error(55, &[Arg::from(r)]);
                    } else {
                        self.baseexp[r] = Some(e);
                        self.eol(s);
                    }
                } else {
                    self.cpu_error(4, &[]);
                }
            } else {
                self.cpu_error(15, &[Arg::C(b',')]);
            }
            return self.skip_line(s);
        } else if len == 4 && eq_nocase(l, name, b"endb") {
            s = skip(&self.line, s);
            let reg = self.getreg(&mut s, false, false);
            if (REGAn..=REGAn + 6).contains(&reg) {
                let r = reg_get(reg) as usize;
                if self.baseexp[r].is_some() {
                    self.baseexp[r] = None;
                    self.eol(s);
                } else {
                    self.cpu_error(56, &[Arg::from(r)]);
                }
            } else {
                self.cpu_error(4, &[]);
            }
            return self.skip_line(s);
        } else if len == 7 && eq_nocase(l, name, b"machine") {
            s = skip(&self.line, s);
            if eq_nocase(&self.line, s, b"mc") {
                s += 2;
            }
            match self.get_cpu_type(&mut s) {
                Some(cpu) => {
                    self.set_cpu_type(cpu, true);
                    self.eol(s);
                }
                None => self.cpu_error(43, &[]),
            }
            return self.skip_line(s);
        } else if len == 7 && eq_nocase(l, name, b"mc") {
            s = name + 2;
            match self.get_cpu_type(&mut s) {
                Some(cpu) => {
                    self.set_cpu_type(cpu, true);
                    self.eol(s);
                }
                None => self.cpu_error(43, &[]),
            }
            return self.skip_line(s);
        } else if len == 5 && eq_nocase(l, name, b"cpu32") {
            self.set_cpu_type(cpu32, true);
            self.eol(s);
            return self.skip_line(s);
        } else if len == 3 && eq_nocase(l, name, b"fpu") {
            s = skip(&self.line, s);
            let mut id: Taddr = 1;
            if self.validchar(s) != 0 {
                id = self.parse_constexpr(&mut s);
            }
            if id != 0 {
                self.add_cpu_opt(None, OCMD_FPU, id);
                self.cpu.cpu_type |= m68881;
            } else {
                self.cpu.cpu_type &= !m68881;
            }
            let ct = self.cpu.cpu_type as i32;
            self.add_cpu_opt(None, OCMD_CPU, ct);
            self.eol(s);
            return self.skip_line(s);
        } else if len == 3 && eq_nocase(l, name, b"opt") {
            let mut s2 = s;
            loop {
                let p = skip(&self.line, s2);
                match self.devpac_option(p) {
                    Some(n) => s2 = skip(&self.line, n),
                    None => break,
                }
                let c = self.line[s2];
                s2 += 1;
                if c != b',' {
                    break;
                }
            }
            return self.skip_line(s);
        }
        start
    }

    /// parse_cpu_label(): label EQUR/REG; returns true when handled
    pub fn parse_cpu_label(&mut self, labname: &str, start: &mut usize) -> bool {
        let dir = *start;
        let mut s = dir;
        if !is_id_start(self.line[s]) {
            return false;
        }
        s += 1;
        while is_id_char(self.line[s], self.opts.local_dots) {
            s += 1;
        }
        let len = s - dir;
        if (len == 4 && eq_nocase(&self.line, dir, b"equr")) || (len == 5 && eq_nocase(&self.line, dir, b"fequr")) {
            s = skip(&self.line, s);
            let r = self.getreg(&mut s, false, false);
            if r >= 0 {
                if self.regsyms.contains_key(labname) && !self.opts.regsymredef {
                    self.general_error(58, &[Arg::from(labname)]);
                } else {
                    self.regsyms.insert(labname.to_string(), RegSym { rtype: if reg_is_an(r) { 1 } else { 0 }, reg: reg_get(r) as i32 });
                }
            } else {
                self.cpu_error(44, &[]);
            }
            self.eol(s);
            *start = self.skip_line(s);
            return true;
        } else if (len == 3 && eq_nocase(&self.line, dir, b"reg")) || (len == 5 && eq_nocase(&self.line, dir, b"equrl")) {
            s = skip(&self.line, s);
            let l = self.scan_rnlist(&mut s);
            self.new_equate(labname, Expr::Num(l as Taddr));
            self.eol(s);
            *start = self.skip_line(s);
            return true;
        }
        false
    }
}
