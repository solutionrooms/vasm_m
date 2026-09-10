//! Operand/instruction optimisation and sizing, transliterated from
//! vasm 1.7h cpus/m68k/cpu.c (eval_oper, optimize_oper, optypes_subset,
//! optimize_instruction, oper_size, iplist_size, instruction_size).
//! Only the branches reachable with a plain 68000 are ported; 020+/ColdFire/
//! FPU-only paths are omitted (they are guarded by cpu_type tests in vasm).
use super::parse::{branch_size, reg_get, reg_is_an, REGAn};
use super::tables::*;
use super::*;
use crate::asm::Assembler;
use crate::atoms::RESOLVE_WARN;
use crate::errors::Arg;
use crate::expr::{Base as EBase, Expr, Op};
use crate::types::*;
use std::rc::Rc;

#[inline]
fn cntones(v: Taddr, bits: u32) -> i32 {
    let mut r = 0;
    let mut x = v as u32;
    for _ in 0..bits {
        r += (x & 1) as i32;
        x >>= 1;
    }
    r
}

#[inline]
fn bfffo(v: Taddr, start: i32, bits: i32) -> i32 {
    let mut i = start;
    while i < bits {
        if v & (1 << i) != 0 {
            break;
        }
        i += 1;
    }
    i
}

/// optypes_subset(): true when mold's operand types are a subset of mnew's
pub fn optypes_subset(mold: &Mnemonic, mnew: &Mnemonic) -> bool {
    for i in 0..MAX_OPERANDS {
        let ot_old = mold.operand_type[i] as usize;
        let ot_new = mnew.operand_type[i] as usize;
        let fl_old = OPTYPES[ot_old].flags;
        let fl_new = OPTYPES[ot_new].flags;
        let m = OPTYPES[ot_old].modes;
        if (ot_old == 0 && ot_new != 0) || (ot_new == 0 && ot_old != 0) {
            return false;
        }
        if (OPTYPES[ot_new].modes & m) != m || (fl_old as u32 & FL_CheckMask) != (fl_new as u32 & FL_CheckMask) {
            return false;
        }
        if fl_old & OTF_SPECREG != 0 && fl_new & OTF_SPECREG != 0 {
            // vasm's break here is outside the if: loop terminates after this slot
            if OPTYPES[ot_old].first < OPTYPES[ot_new].first || OPTYPES[ot_old].last > OPTYPES[ot_new].last {
                return false;
            }
            return true;
        }
    }
    true
}

fn aindir_in_list(op: &Operand, list: Taddr) -> bool {
    if op.mode == MODE_AnPostInc || op.mode == MODE_AnPreDec {
        return list & (1 << (REGAn + op.reg as i32)) != 0;
    }
    false
}

fn ip_doubleop(code: i32, q: u8, mode1: i8, reg1: i8, flags1: u32, exp1: Option<Rc<Expr>>, mode2: i8, reg2: i8, flags2: u32) -> Instruction {
    let mut ip = Instruction::new(code);
    ip.qual = q;
    let mut o = Operand::new();
    o.mode = mode1;
    o.reg = reg1;
    o.flags = flags1;
    o.value[0] = exp1;
    ip.op[0] = Some(o);
    if mode2 >= 0 {
        let mut o2 = Operand::new();
        o2.mode = mode2;
        o2.reg = reg2;
        o2.flags = flags2;
        ip.op[1] = Some(o2);
    }
    ip.ext.last_size = -1;
    ip.ext.orig_ext = -1;
    ip.ext.flags = 0;
    ip
}

fn ip_singleop(code: i32, q: u8, mode: i8, reg: i8, flags: u32, exp: Option<Rc<Expr>>) -> Instruction {
    ip_doubleop(code, q, mode, reg, flags, exp, -1, 0, 0)
}

impl Assembler {
    /// eval_oper()
    fn eval_oper(&mut self, op: &mut Operand, sec: usize, pc: Taddr, final_: bool) {
        for i in 0..2 {
            op.base[i] = None;
            let ty = match &op.value[i] {
                Some(v) => self.type_of_expr(v),
                None => 0,
            };
            if ty == 1 {
                loop {
                    let v = op.value[i].clone().unwrap();
                    let (val, cnst) = self.eval_expr(&v, Some(sec), pc);
                    op.extval[i] = val;
                    if !cnst {
                        let (bt, base) = self.find_base(&v, Some(sec), pc);
                        op.basetype[i] = bt;
                        op.base[i] = base;
                        if bt == EBase::Illegal {
                            if op.flags & FL_BaseReg != 0 && self.fix_basereg(op) {
                                continue;
                            }
                            if final_ {
                                self.general_error(38, &[]);
                            }
                        }
                    }
                    break;
                }
                op.flags |= FL_ExtVal0 << i;
            } else {
                op.extval[i] = 0x7fffffff;
                op.flags &= !(FL_ExtVal0 << i);
            }
        }
    }

    /// optimize_oper()
    fn optimize_oper(&mut self, op: &mut Operand, ot: &OpType, sec: usize, pc: Taddr, cpc: Taddr, final_: bool) {
        if op.flags & FL_DoNotEval == 0 {
            self.eval_oper(op, sec, pc, final_);
        }
        if op.flags & FL_NoOpt == FL_NoOpt {
            return;
        }
        let bopt = op.flags & FL_NoOptBase == 0;
        let size16_0 = op.extval[0] >= -0x8000 && op.extval[0] <= 0x7fff;
        let pcdisp = op.extval[0].wrapping_sub(cpc);
        let pcdisp16 = match op.base[0] {
            None => false,
            Some(_) => pcdisp >= -0x8000 && pcdisp <= 0x7fff,
        };
        let undef = match op.base[0] {
            None => false,
            Some(b) => self.symtab.syms[b].is_extref(),
        };
        let c = &self.cpu;
        let is020 = c.cpu_type & (m68020up | cpu32) != 0;
        if bopt {
            if op.mode == MODE_An16Disp {
                if c.opt_disp && op.base[0].is_none() && op.extval[0] == 0 && ot.modes & (1 << 2) != 0 {
                    op.mode = MODE_AnIndir;
                    if final_ && c.warn_opts > 1 {
                        self.cpu_error(49, &[Arg::from("(0,An)->(An)")]);
                    }
                    op.value[0] = None;
                }
                // (d16,An) -> (bd32,An,ZDn.w): 020+ only
            } else if op.mode == MODE_Extended && op.reg == REG_PC16Disp && is020 {
                // 020+ only
            } else if op.mode == MODE_An8Format && op.flags & FL_UsesFormat != 0 && op.format & FW_FullFormat == 0 {
                // 020+ only
            } else if op.mode == MODE_Extended && op.reg == REG_PC8Format && op.flags & FL_UsesFormat != 0 && op.format & FW_FullFormat == 0 {
                // 020+ only
            } else if op.mode == MODE_Extended && op.reg == REG_AbsShort && ot.modes & (1 << 8) != 0 {
                if op.base[0].is_none() && !size16_0 {
                    op.reg = REG_AbsLong;
                    if final_ && c.warn_opts > 1 {
                        self.cpu_error(50, &[Arg::from("abs.w->abs.l")]);
                    }
                } else if let Some(b) = op.base[0] {
                    if c.typechk && self.symtab.syms[b].is_locref() {
                        op.reg = REG_AbsLong;
                        if final_ {
                            self.cpu_error(22, &[]);
                        }
                    }
                }
            } else if op.mode == MODE_Extended && op.reg == REG_AbsLong {
                if c.opt_abs && op.base[0].is_none() && size16_0 && ot.modes & (1 << 7) != 0 {
                    op.reg = REG_AbsShort;
                    if final_ && c.warn_opts > 1 {
                        self.cpu_error(49, &[Arg::from("abs.l->abs.w")]);
                    }
                } else if c.opt_pc && op.base[0].is_some() && ot.modes & (1 << 9) != 0 {
                    let b = op.base[0].unwrap();
                    if !undef && pcdisp16 && self.symtab.syms[b].sec == Some(sec) {
                        op.reg = REG_PC16Disp;
                        if final_ && self.cpu.warn_opts > 1 {
                            self.cpu_error(49, &[Arg::from("label->(d16,PC)")]);
                        }
                    }
                }
            }
        }
        // full-format word optimisations are 020+ only; unreachable on 68000
    }

    pub fn optimize_instruction_pub(&mut self, ip: &mut Instruction, sec: usize, pc: Taddr, final_: bool) -> u8 {
        self.optimize_instruction(ip, sec, pc, final_)
    }

    /// optimize_instruction(): returns ipflags
    fn optimize_instruction(&mut self, iplist: &mut Instruction, sec: usize, pc: Taddr, final_: bool) -> u8 {
        let mut ext = iplist.qual;
        let mut ipflags = iplist.ext.flags;
        let lastsize = iplist.ext.last_size;
        let _orig_ext = iplist.ext.orig_ext;
        let ct = self.cpu.cpu_type;
        // stage D: generalise mnemonic
        loop {
            let code = iplist.code as usize;
            if code + 1 >= MNEMONICS.len() {
                break;
            }
            let mnemo = &MNEMONICS[code];
            let nextmn = &MNEMONICS[code + 1];
            if mnemo.name != nextmn.name || nextmn.available & ct == 0 {
                break;
            }
            let mut nextsize = nextmn.size;
            if (mnemo.size & SIZE_MASK) != SIZE_UNSIZED || (nextsize & SIZE_MASK) != SIZE_UNSIZED {
                if nextsize & S_CFCHECK != 0 && ct & mcf != 0 {
                    nextsize &= !(SIZE_BYTE | SIZE_WORD);
                }
                if nextsize & lc_ext_to_size(ext) == 0 {
                    break;
                }
            }
            if !optypes_subset(mnemo, nextmn) {
                break;
            }
            iplist.code += 1;
        }
        let mut mnemo = MNEMONICS[iplist.code as usize];
        let cpc = pc.wrapping_add(((mnemo.size & 3) as Taddr) << 1);
        let pc = if self.cpu.phxass_compat { cpc } else { pc };
        // JMP/JSR (label,PC) is never optimised
        if (mnemo.opcode[0] == 0x4ec0 || mnemo.opcode[0] == 0x4e80) {
            if let Some(o) = &mut iplist.op[0] {
                if o.mode == MODE_Extended && o.reg == REG_PC16Disp {
                    o.flags |= FL_NoOpt;
                }
            }
        }
        // evaluate and optimise operands
        for i in 0..MAX_OPERANDS {
            let ot = OPTYPES[mnemo.operand_type[i] as usize];
            match iplist.op[i].as_mut() {
                Some(o) => {
                    let mut o2 = std::mem::replace(o, Operand::new());
                    self.optimize_oper(&mut o2, &ot, sec, pc, cpc, final_);
                    iplist.op[i] = Some(o2);
                }
                None => break,
            }
        }
        // MOVEM register list on the wrong side: swap
        if mnemo.name == "movem" && mnemo.place[0] == PL_D16 && mnemo.place[1] == PL_SEA
            && iplist.op[0].as_ref().unwrap().base[0].is_some() && iplist.op[1].as_ref().unwrap().base[0].is_none()
        {
            iplist.op[0].as_mut().unwrap().reg = REG_AbsLong;
            iplist.op[1].as_mut().unwrap().reg = REG_RnList;
            iplist.code += 2;
            mnemo = MNEMONICS[iplist.code as usize];
        }
        iplist.next = None;
        if self.cpu.no_opt {
            return ipflags;
        }
        let c = self.cpu.clone();
        let oc = self.oc();
        // STAGE 1 (opt_mul only; opt_speed variants omitted)
        let mut opc = mnemo.opcode[0];
        let (mut val, mut abs) = match &iplist.op[0] {
            Some(o) => (o.extval[0], o.base[0].is_none()),
            None => (0, false),
        };
        let ip = iplist; // stage 2 always looks at the first instruction here
        if c.opt_mul && abs && (opc == 0xc0c0 || opc == 0xc1c0 || opc == 0x4c00) && mnemo.opcode[1] & 0x0400 == 0
            && op_is_imm(ip, 0)
        {
            let muls = (opc & 0x0100) != 0 || (mnemo.opcode[1] & 0x0800) != 0;
            if val == 0 {
                ip.code = oc.moveq;
                ip.qual = b'l';
            } else if val == 1 {
                if ext == b'w' && muls {
                    ip.code = oc.ext;
                    ip.qual = b'l';
                    ip.op[0] = ip.op[1].take();
                } else if ext == b'l' {
                    ip.code = oc.tst;
                    ip.op[0] = ip.op[1].take();
                }
            } else if val == -1 && muls {
                if ext != b'w' {
                    ip.code = oc.neg;
                    ip.op[0] = ip.op[1].take();
                }
            } else if (2..=0x100).contains(&val) && cntones(val, 9) == 1 {
                let v = bfffo(val, 1, 9);
                if ext == b'l' {
                    ip.code = if muls { oc.asli } else { oc.lsli };
                    let o = ip.op[0].as_mut().unwrap();
                    if final_ {
                        o.value[0] = Some(Rc::new(Expr::Num(v)));
                    } else {
                        o.flags |= FL_DoNotEval;
                    }
                    o.extval[0] = v;
                }
            }
        }
        // STAGE 2
        if ip.code >= 0 {
            mnemo = MNEMONICS[ip.code as usize];
            opc = mnemo.opcode[0];
            ext = ip.qual;
        }
        let is_move = ip.code == oc.mov || opc == 0x0040;
        if is_move && op_is_imm(ip, 0) {
            let dst_mode = ip.op[1].as_ref().map(|o| o.mode).unwrap_or(-1);
            let dst_reg = ip.op[1].as_ref().map(|o| o.reg).unwrap_or(-1);
            if c.opt_moveq && abs && ext == b'l' && val >= -0x80 && val <= 0x7f && dst_mode == MODE_Dn {
                ip.code = oc.moveq;
                ip.qual = b'l';
            } else if c.opt_gen && abs && val == 0 && opc & 0x0040 == 0 && (ct & (m68010up | mcf | cpu32) != 0 || c.opt_clr) {
                ip.code = oc.clr;
                ip.op[0] = ip.op[1].take();
            } else if c.opt_moveq && abs && ext == b'l' && (val == -1 || (1..=7).contains(&val)) && ct & (mcfb | mcfc) != 0 {
                ip.code = oc.mov3q;
            } else if c.opt_st && abs && ext == b'b' && opc & 0x0040 == 0 && (val & 0xff) == 0xff {
                if ct & mcf == 0 || dst_mode == MODE_Dn {
                    ip.code = oc.st;
                    ip.op[0] = ip.op[1].take();
                }
            } else if c.opt_pea && dst_mode == MODE_AnPreDec && dst_reg == 7 && ext == b'l' {
                if abs && val >= -0x8000 && val <= 0x7fff {
                    ip.op[0].as_mut().unwrap().reg = REG_AbsShort;
                    ip.code = oc.pea;
                    ip.op[1] = None;
                } else if ct & (m68000 | m68010 | m68020 | m68030 | cpu32) != 0 {
                    ip.op[0].as_mut().unwrap().reg = REG_AbsLong;
                    ip.code = oc.pea;
                    ip.op[1] = None;
                }
            } else if c.opt_gen && opc == 0x0040 {
                if abs && val == 0 {
                    ip.code = oc.suba;
                    ip.qual = b'l';
                    let o = ip.op[0].as_mut().unwrap();
                    o.mode = MODE_An;
                    o.reg = dst_reg;
                } else if !abs && ext == b'l' {
                    ip.code = oc.lea;
                    ip.op[0].as_mut().unwrap().reg = REG_AbsLong;
                }
            }
        } else if c.opt_moveq && abs && (opc & 0xff7f) == 0x7100 && op_is_imm(ip, 0)
            && ((val >= -0x80 && opc & 0x0080 == 0) || val >= 0) && val <= 0x7f
        {
            ip.code = oc.moveq;
            ip.qual = b'l';
        } else if (c.opt_gen || c.opt_movem) && mnemo.name == "movem" {
            let o = if opc & 0x0400 != 0 { 1 } else { 0 };
            let (mode, reg, base_none, noopt, list) = {
                let op = ip.op[o].as_ref().unwrap();
                (op.mode, op.reg, op.base[0].is_none(), op.flags & FL_NoOpt != 0, op.extval[0])
            };
            if mode == MODE_Extended && (reg == REG_RnList || reg == REG_Immediate) && base_none && !noopt {
                let regs = cntones(list, 16);
                if regs == 0 {
                    ip.code = -1;
                } else if regs == 1 {
                    let other = ip.op[o ^ 1].as_ref().unwrap();
                    if (c.opt_movem || (list & 0xff == 0 && o == 1)) && !aindir_in_list(other, list) {
                        let r = bfffo(list, 0, 16);
                        ip.code = oc.mov;
                        let op = ip.op[o].as_mut().unwrap();
                        op.mode = if reg_is_an(r) { MODE_An } else { MODE_Dn };
                        op.reg = reg_get(r);
                    }
                }
                // regs==2 with opt_speed: 68020+/68040 only
            }
        } else if c.opt_gen && opc == 0x4200 && ip.op[0].as_ref().unwrap().mode == MODE_Dn && ext == b'l' {
            ip.code = oc.moveq;
            ip.qual = b'l';
            if final_ {
                ip.op[1] = ip.op[0].take();
                let mut o = Operand::new();
                o.mode = MODE_Extended;
                o.reg = REG_Immediate;
                o.value[0] = Some(Rc::new(Expr::Num(0)));
                ip.op[0] = Some(o);
            } else {
                ip.op[0] = None;
            }
        } else if c.opt_gen && abs && (opc == 0xc000 || opc == 0x0200) && op_is_imm(ip, 0) {
            let dst_mode = ip.op[1].as_ref().map(|o| o.mode).unwrap_or(-1);
            if ct & (mcfb | mcfc) != 0 && dst_mode == MODE_Dn && ext == b'l' && (val == 0xff || val == 0xffff) {
                // ColdFire only
            } else if (val == 0xff && ext == b'b') || (val == 0xffff && ext == b'w') || (val == -1 && ext == b'l') {
                ip.code = oc.tst;
                ip.op[0] = ip.op[1].take();
            } else if val == 0 && (ct & (m68010up | mcf | cpu32) != 0 || c.opt_clr) {
                ip.code = oc.clr;
                ip.op[0] = ip.op[1].take();
            }
        } else if c.opt_gen && abs && val == 0 && (opc == 0x8000 || opc == 0x0000 || opc == 0x0a00) && op_is_imm(ip, 0) {
            ip.code = oc.tst;
            ip.op[0] = ip.op[1].take();
        } else if c.opt_gen && abs && opc == 0x0a00 {
            if (ext == b'b' && (val & 0xff) == 0xff) || (ext == b'w' && (val & 0xffff) == 0xffff) || (ext == b'l' && val == -1) {
                ip.code = oc.not;
                ip.op[0] = ip.op[1].take();
            }
        } else if opc == 0x0600 || opc == 0xd000 || opc == 0xd0c0 || opc == 0x0400 || opc == 0x9000 || opc == 0x90c0 {
            if op_is_imm(ip, 0) && abs {
                if c.opt_quick && (1..=8).contains(&val) {
                    ip.code = if opc & 0x4200 != 0 { oc.addq } else { oc.subq };
                } else if (opc & 0x90c0) == 0x90c0 {
                    let mut v = val;
                    if opc & 0x4000 == 0 {
                        v = v.wrapping_neg();
                    }
                    if c.opt_gen && v == 0 {
                        ip.code = -1;
                    } else if c.opt_lea && v >= -0x8000 && v <= 0x7fff {
                        ip.qual = b'l';
                        ip.code = oc.lea;
                        let dst_reg = ip.op[1].as_ref().unwrap().reg;
                        let o = ip.op[0].as_mut().unwrap();
                        o.mode = MODE_An16Disp;
                        o.reg = dst_reg;
                        if opc & 0x4000 == 0 && final_ {
                            o.value[0] = Some(Rc::new(Expr::Num(v)));
                        } else {
                            o.flags |= FL_DoNotEval;
                            o.extval[0] = v;
                        }
                    }
                }
            }
        } else if opc == 0x41c0 {
            let (m0, r0) = { let o = ip.op[0].as_ref().unwrap(); (o.mode, o.reg) };
            let r1 = ip.op[1].as_ref().unwrap().reg;
            if m0 == MODE_An16Disp && abs {
                if c.opt_gen && r0 == r1 && val == 0 {
                    ip.code = -1;
                } else if c.opt_lquick && r0 == r1 && val != 0 && (-8..=8).contains(&val) {
                    if val < 0 {
                        ip.code = oc.subq;
                        let v = -val;
                        if final_ {
                            ip.op[0].as_mut().unwrap().value[0] = Some(Rc::new(Expr::Num(v)));
                        }
                    } else {
                        ip.code = oc.addq;
                    }
                    ip.qual = b'l';
                    let o = ip.op[0].as_mut().unwrap();
                    o.mode = MODE_Extended;
                    o.reg = REG_Immediate;
                } else if c.opt_gen && (val < -0x8000 || val > 0x7fff) && ct & (m68020up | cpu32) == 0 {
                    if r0 == r1 {
                        ip.code = oc.adda;
                        let o = ip.op[0].as_mut().unwrap();
                        o.mode = MODE_Extended;
                        o.reg = REG_Immediate;
                        o.flags |= FL_NoOpt;
                    } else {
                        let v0 = ip.op[0].as_ref().unwrap().value[0].clone();
                        let ip2 = ip_doubleop(oc.adda, b'l', MODE_Extended, REG_Immediate, FL_NoOpt, v0, MODE_An, r1, FL_NoOpt);
                        ip.code = oc.movea;
                        ip.op[0].as_mut().unwrap().mode = MODE_An;
                        ip.next = Some(Box::new(ip2));
                    }
                    ip.qual = b'l';
                    if final_ {
                        self.cpu_error(47, &[]);
                    }
                }
            } else if c.opt_gen && m0 == MODE_AnIndir && r0 == r1 {
                ip.code = -1;
            } else if c.opt_gen && abs && val == 0 && m0 == MODE_Extended && (r0 == REG_AbsShort || r0 == REG_AbsLong) {
                ip.code = oc.suba;
                ip.qual = b'l';
                let o = ip.op[0].as_mut().unwrap();
                o.mode = MODE_An;
                o.reg = r1;
            }
        } else if c.opt_gen && opc == 0x4808 && ip.op[1].as_ref().unwrap().base[0].is_none() {
            // LINK.L -> LINK.W (020+ mnemonic; unreachable on 68000)
            let v = ip.op[1].as_ref().unwrap().extval[0];
            if v >= -0x8000 && v <= 0x7fff {
                ip.qual = b'w';
                ip.code -= 1;
            }
        } else if opc == 0x0c00 || opc == 0xb000 || opc == 0xb0c0 {
            if c.opt_gen && abs && val == 0 && op_is_imm(ip, 0) {
                if opc != 0xb0c0 || ct & (m68020up | cpu32 | mcf) != 0 {
                    if opc == 0xb0c0 {
                        ip.code = oc.tst + 1;
                        ip.qual = b'l';
                    } else {
                        ip.code = oc.tst;
                    }
                    ip.op[0] = ip.op[1].take();
                }
            }
        } else if (((opc & 0xf1ff) == 0xe100 && c.opt_gen) || ((opc & 0xf1ff) == 0xe108 && c.opt_lsl)) && ct & (m68060 | mcf) == 0 {
            if (opc & 0x0e00) == 0x0200 {
                val = 1;
            }
            if val == 1 {
                ip.code = oc.add;
                let r1 = ip.op[1].as_ref().map(|o| o.reg).unwrap_or(0);
                let o = ip.op[0].as_mut().unwrap();
                o.mode = MODE_Dn;
                if opc & 0x0e00 == 0 {
                    o.reg = r1;
                }
                // note: for "asl Dn" (opc&0x0e00==0x0200) vasm leaves reg unchanged (register-only form: op[0] is Dn already)
            }
            // opt_speed && opt_lsl && val==2: omitted (opt_speed off)
        } else if (opc & 0xfeff) == 0x80c0 && abs && op_is_imm(ip, 0) {
            if val == 0 {
                if final_ {
                    self.cpu_error(60, &[]);
                }
            }
        } else if opc == 0x4c40 && abs && op_is_imm(ip, 0) {
            if val == 0 {
                if final_ {
                    self.cpu_error(60, &[]);
                }
            } else if c.opt_div && mnemo.operand_type[1] == D_ {
                if val == 1 {
                    ip.code = oc.tst;
                    ip.op[0] = ip.op[1].take();
                } else if val == -1 && mnemo.opcode[1] & 0x0800 != 0 {
                    ip.code = oc.neg;
                    ip.op[0] = ip.op[1].take();
                } else if (2..=0x100).contains(&val) && mnemo.opcode[1] & 0x0800 == 0 && cntones(val, 9) == 1 {
                    ip.code = oc.lsri;
                    let v = bfffo(val, 1, 9);
                    let o = ip.op[0].as_mut().unwrap();
                    if final_ {
                        o.value[0] = Some(Rc::new(Expr::Num(v)));
                    } else {
                        o.flags |= FL_DoNotEval;
                    }
                    o.extval[0] = v;
                }
            }
        } else if (opc == 0x4ec0 || opc == 0x4e80) && !abs {
            let (noopt, mode, reg, base) = {
                let o = ip.op[0].as_ref().unwrap();
                (o.flags & FL_NoOpt != 0, o.mode, o.reg, o.base[0])
            };
            let base_ok = match base {
                Some(b) => self.symtab.syms[b].is_locref() && self.symtab.syms[b].sec == Some(sec),
                None => false,
            };
            if c.opt_pc && !noopt && mode == MODE_Extended && (reg == REG_AbsLong || reg == REG_PC16Disp) && base_ok {
                let diff = val.wrapping_sub(cpc);
                if lastsize == 0 || (diff == 0 && opc & 0x40 != 0) {
                    ip.code = -1;
                } else if diff >= -0x8000 && diff <= 0x7fff {
                    if diff >= -0x80 && diff <= 0x7f {
                        if (lastsize == 2 && diff == 0) || (lastsize == 4 && diff == 2) {
                            ip.qual = b'w';
                        } else {
                            ip.qual = b'b';
                        }
                    } else {
                        ip.qual = b'w';
                    }
                    ip.code = if opc & 0x40 != 0 { oc.bra } else { oc.bsr };
                    ip.op[0].as_mut().unwrap().reg = REG_AbsLong;
                }
            } else if c.opt_sc && !noopt && mode == MODE_Extended && reg == REG_AbsLong && base.map(|b| self.symtab.syms[b].is_extref()).unwrap_or(false) {
                ip.op[0].as_mut().unwrap().reg = REG_PC16Disp;
            }
        } else if (opc & 0xf000) == 0x6000 && !abs {
            let base = ip.op[0].as_ref().unwrap().base[0];
            let base_ok = match base {
                Some(b) => self.symtab.syms[b].is_locref() && self.symtab.syms[b].sec == Some(sec),
                None => false,
            };
            if c.opt_bra && (ipflags & IFL_UNSIZED != 0 || c.opt_allbra) && base_ok {
                let diff = val.wrapping_sub(cpc);
                let resolvewarn = self.sections[sec].flags & RESOLVE_WARN != 0;
                match lastsize {
                    0 => {
                        if diff != -2 { ip.qual = b'b'; } else { ip.code = -1; }
                    }
                    2 => {
                        if diff == 0 && opc != 0x6100 && !resolvewarn {
                            ip.code = -1;
                        } else if diff < -0x80 || diff > 0x7f || diff == 0 {
                            ip.qual = b'w';
                        } else {
                            ip.qual = b'b';
                        }
                    }
                    4 => {
                        if diff == 2 {
                            if opc != 0x6100 && !resolvewarn { ip.code = -1; } else { ip.qual = b'w'; }
                        } else if diff >= -0x80 && diff <= 0x80 && !resolvewarn {
                            ip.qual = b'b';
                        } else if diff < -0x8000 || diff > 0x7fff {
                            if ct & (m68020up | cpu32 | mcfb | mcfc) != 0 {
                                ip.qual = b'l';
                            } else {
                                ip.qual = 0;
                                ipflags |= IFL_RETAINLASTSIZE;
                                if opc < 0x6200 {
                                    ip.code = if opc == 0x6000 { oc.jmp } else { oc.jsr };
                                    if final_ {
                                        self.cpu_error(46, &[]);
                                    }
                                } else {
                                    let v0 = ip.op[0].as_ref().unwrap().value[0].clone();
                                    let ip2 = ip_singleop(oc.jmp, 0, MODE_Extended, REG_AbsLong, FL_NoOpt, v0);
                                    ip.code += if opc & 0x0100 != 0 { -2 } else { 2 };
                                    ip.qual = b'b';
                                    ip.op[0].as_mut().unwrap().flags |= FL_NoOpt;
                                    ip.next = Some(Box::new(ip2));
                                    if final_ {
                                        let cpcs = self.curpc_sym();
                                        let e = Expr::Bin(Op::Add, Box::new(Expr::Sym(cpcs)), Box::new(Expr::Num(if self.cpu.phxass_compat { 6 } else { 8 })));
                                        ip.op[0].as_mut().unwrap().value[0] = Some(Rc::new(e));
                                        self.cpu_error(46, &[]);
                                    }
                                }
                            }
                        } else {
                            ip.qual = b'w';
                        }
                    }
                    6 => {
                        if diff >= -0x8000 && diff <= 0x7fff && !resolvewarn { ip.qual = b'w'; } else { ip.qual = b'l'; }
                    }
                    _ => {
                        if ext == 0 {
                            ip.qual = b'w';
                        }
                    }
                }
            } else if c.opt_branop && opc != 0x6100 && val.wrapping_sub(cpc) == 0 && (ext == b'b' || ext == b's') && base_ok {
                ip.qual = 0;
                ip.code = oc.noop;
                ip.op[0] = None;
                if final_ {
                    self.cpu_error(57, &[]);
                }
            } else if c.opt_brajmp && base.map(|b| self.symtab.syms[b].sec != Some(sec) && self.symtab.syms[b].is_locref()).unwrap_or(false) {
                ip.qual = 0;
                if opc < 0x6200 {
                    ip.code = if opc == 0x6000 { oc.jmp } else { oc.jsr };
                } else {
                    let v0 = ip.op[0].as_ref().unwrap().value[0].clone();
                    let ip2 = ip_singleop(oc.jmp, 0, MODE_Extended, REG_AbsLong, FL_NoOpt, v0);
                    ip.code += if opc & 0x0100 != 0 { -2 } else { 2 };
                    ip.qual = b'b';
                    ip.op[0].as_mut().unwrap().flags |= FL_NoOpt;
                    ip.next = Some(Box::new(ip2));
                    if final_ {
                        let cpcs = self.curpc_sym();
                        let e = Expr::Bin(Op::Add, Box::new(Expr::Sym(cpcs)), Box::new(Expr::Num(if self.cpu.phxass_compat { 6 } else { 8 })));
                        ip.op[0].as_mut().unwrap().value[0] = Some(Rc::new(e));
                    }
                }
            }
        }
        // opt_immaddr
        if c.opt_immaddr && abs && ext == b'l' && ip.op[0].is_some() && op_is_imm(ip, 0)
            && ip.op[1].as_ref().map(|o| o.mode == MODE_An).unwrap_or(false)
            && val >= -0x8000 && val <= 0x7fff && ct & mcf == 0
            && ip.code >= 0
            && MNEMONICS[ip.code as usize].size & SIZE_WORD != 0
            && (MNEMONICS[ip.code as usize].opcode[0] & 0xfeff) != 0x5000
        {
            ip.qual = b'w';
        }
        // optimise operands again over the whole chain
        let mut cur: Option<&mut Instruction> = Some(ip);
        while let Some(i) = cur {
            if i.code >= 0 {
                let m = MNEMONICS[i.code as usize];
                for k in 0..MAX_OPERANDS {
                    let ot = OPTYPES[m.operand_type[k] as usize];
                    match i.op[k].as_mut() {
                        Some(o) => {
                            let mut o2 = std::mem::replace(o, Operand::new());
                            self.optimize_oper(&mut o2, &ot, sec, pc, cpc, final_);
                            i.op[k] = Some(o2);
                        }
                        None => break,
                    }
                }
            }
            cur = i.next.as_deref_mut();
        }
        let _ = abs;
        ipflags
    }

    #[inline]
    fn oc(&self) -> OpCodes {
        self.cpu.oc
    }

    /// oper_size()
    fn oper_size(ip: &Instruction, op: &Operand, ot: &OpType) -> usize {
        let mode = op.mode;
        let reg = op.reg;
        if ot.flags & OTF_NOSIZE != 0 {
            0
        } else if mode == MODE_An16Disp || (mode == MODE_Extended && (reg == REG_PC16Disp || reg == REG_AbsShort)) {
            2
        } else if mode == MODE_Extended && reg == REG_AbsLong {
            if ot.flags & OTF_BRANCH != 0 { branch_size(ip.qual) } else { 4 }
        } else if mode == MODE_Extended && reg == REG_Immediate {
            match ip.qual {
                b'b' | b'w' => 2,
                b'l' | b's' => 4,
                b'q' | b'd' => 8,
                b'x' | b'p' => 12,
                _ => 0,
            }
        } else if mode == MODE_An8Format || (mode == MODE_Extended && reg == REG_PC8Format) {
            let mut n = 2;
            if op.format & FW_FullFormat != 0 {
                match op.format & FW_BDSize_Mask {
                    0x20 => n += 2,
                    0x30 => n += 4,
                    _ => {}
                }
                match op.format & FW_IndSize_Mask {
                    2 => n += 2,
                    3 => n += 4,
                    _ => {}
                }
            }
            n
        } else {
            0
        }
    }

    /// iplist_size()
    pub fn iplist_size(ip: &Instruction) -> usize {
        let mut size = 0;
        let mut cur = Some(ip);
        while let Some(i) = cur {
            if i.code >= 0 {
                let m = &MNEMONICS[i.code as usize];
                size += ((m.size & 3) as usize) << 1;
                for k in 0..MAX_OPERANDS {
                    match &i.op[k] {
                        Some(o) => size += Self::oper_size(i, o, &OPTYPES[m.operand_type[k] as usize]),
                        None => break,
                    }
                }
            }
            cur = i.next.as_deref();
        }
        size
    }

    /// instruction_size()
    pub fn instruction_size(&mut self, realip: &mut Instruction, sec: usize, pc: Taddr) -> usize {
        let ct = self.cpu.cpu_type;
        let mut ext = realip.qual;
        // stage B: cpu availability
        while MNEMONICS[realip.code as usize].available & ct == 0 {
            let lastm = MNEMONICS[realip.code as usize];
            let next = MNEMONICS[realip.code as usize + 1];
            if lastm.name != next.name || !optypes_subset(&lastm, &next) {
                self.cpu_error(0, &[]);
            }
            realip.code += 1;
        }
        let mut mnemo = MNEMONICS[realip.code as usize];
        let mut extsize = mnemo.size;
        if self.cpu.opt_allbra && self.cpu.ign_unambig_ext {
            for i in 0..MAX_OPERANDS {
                if mnemo.operand_type[i] == BR {
                    ext = 0;
                    realip.qual = 0;
                    break;
                }
            }
        }
        if ext == 0 {
            realip.ext.flags |= IFL_UNSIZED;
            if extsize & SIZE_MASK != 0 {
                if extsize & S_CFCHECK != 0 && ct & mcf != 0 {
                    extsize &= !(SIZE_BYTE | SIZE_WORD);
                }
                realip.qual = if extsize & SIZE_LONG != 0 && ct & mcf != 0 { b'l' }
                    else if extsize & SIZE_WORD != 0 { b'w' }
                    else if extsize & SIZE_BYTE != 0 { b'b' }
                    else if extsize & SIZE_LONG != 0 { b'l' }
                    else if extsize & SIZE_EXTENDED != 0 { b'x' }
                    else if extsize & SIZE_SINGLE != 0 { b's' }
                    else if extsize & SIZE_DOUBLE != 0 { b'd' }
                    else if extsize & SIZE_PACKED != 0 { b'p' }
                    else { self.ierror(0, "optimize.rs", line!()) };
                ext = realip.qual;
            }
        }
        if mnemo.size & SIZE_MASK == SIZE_UNSIZED {
            if ext != 0 {
                if !self.cpu.ign_unambig_ext {
                    self.cpu_error(35, &[]);
                }
                ext = 0;
                realip.qual = 0;
            }
        } else {
            let mut err = false;
            let mut extsize = mnemo.size;
            let sz = lc_ext_to_size(ext);
            let mut uacode: i32 = if mn_size(realip.code) & SIZE_UNAMBIG != 0 && mnemo.available & ct != 0 { realip.code } else { -1 };
            let orig = realip.code as usize;
            loop {
                let eff = if extsize & S_CFCHECK != 0 && ct & mcf != 0 { extsize & !(SIZE_BYTE | SIZE_WORD) } else { extsize };
                if eff & sz != 0 {
                    break;
                }
                let ni = realip.code as usize + 1;
                if ni >= MNEMONICS.len() || MNEMONICS[orig].name != MNEMONICS[ni].name {
                    err = true;
                    break;
                }
                if MNEMONICS[orig].operand_type != MNEMONICS[ni].operand_type {
                    err = true;
                    break;
                }
                realip.code += 1;
                mnemo = MNEMONICS[ni];
                extsize = mnemo.size;
                if mn_size(realip.code) & SIZE_UNAMBIG != 0 && mnemo.available & ct != 0 {
                    uacode = realip.code;
                }
            }
            if err {
                if self.cpu.ign_unambig_ext && uacode >= 0 {
                    realip.code = uacode;
                    mnemo = MNEMONICS[uacode as usize];
                    ext = 0;
                    realip.qual = 0;
                } else {
                    self.cpu_error(34, &[]);
                }
            }
            if mnemo.available & ct == 0 {
                self.cpu_error(0, &[]);
            }
        }
        if realip.ext.orig_ext < 0 {
            realip.ext.orig_ext = ext as i8;
        }
        if mnemo.opcode[0] == 0xf518 && ct & m68040up == 0 {
            realip.code += 1;
        }
        // optimise a copy
        let mut ip = realip.clone();
        ip.next = None;
        let extflags = self.optimize_instruction(&mut ip, sec, pc, false);
        let size = Self::iplist_size(&ip);
        if extflags & IFL_RETAINLASTSIZE == 0 {
            realip.ext.last_size = size as i8;
        }
        size
    }
}

#[inline]
fn op_is_imm(ip: &Instruction, i: usize) -> bool {
    match &ip.op[i] {
        Some(o) => o.mode == MODE_Extended && o.reg == REG_Immediate,
        None => false,
    }
}
