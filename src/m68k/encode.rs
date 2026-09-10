//! Instruction and data encoding, transliterated from vasm 1.7h cpus/m68k/cpu.c
//! (write_val, write_branch, write_extval, write_ea_ext, eval_instruction,
//! eval_data). Relocations are attached to the data block exactly as vasm
//! does (add_extnreloc); -Fbin ignores them, -Fhunk converts them.
use super::tables::*;
use super::*;
use crate::asm::Assembler;
use crate::atoms::{setval_be, DBlock, Reloc, ABSOLUTE, REL_ABS, REL_NONE, REL_PC, REL_SD};
use crate::errors::Arg;
use crate::expr::Base as EBase;
use crate::symbols::ABSLABEL;
use crate::types::*;

fn reverse(v: u32, size: u32) -> Taddr {
    let mut r: u32 = 0;
    for i in 0..size {
        r <<= 1;
        if v & (1u32 << i) != 0 {
            r += 1;
        }
    }
    r as Taddr
}

impl Assembler {
    /// is_pc_reloc(): symbol needs a pc-relative relocation
    fn is_pc_reloc(&mut self, sym: usize, cur_sec: usize) -> bool {
        let s = &self.symtab.syms[sym];
        if s.is_extref() {
            return true;
        }
        if s.is_locref() {
            return s.sec != Some(cur_sec) && (s.flags & ABSLABEL == 0 || self.sections[cur_sec].flags & ABSOLUTE == 0);
        }
        self.ierror(0, "encode.rs", line!())
    }

    /// write_val(): insert `size` bits of val at bit position pos (ORed in)
    fn write_val(&mut self, d: &mut [u8], pos: usize, size: u32, val: Taddr, sign: bool) {
        if self.cpu.typechk {
            if sign {
                let hi = (1i64 << (size - 1)) - 1;
                let lo = -(1i64 << (size - 1));
                if (val as i64) > hi || (val as i64) < lo {
                    if val > 0 && (val as i64) < (1i64 << size) {
                        self.cpu_error(27, &[Arg::from(val as i64), Arg::from(lo), Arg::from(hi), Arg::from(val as i64 - (1i64 << size))]);
                    } else {
                        self.cpu_error(25, &[Arg::from(val as i64), Arg::from(lo), Arg::from(hi)]);
                    }
                }
            } else if (val as Utaddr) as u64 > (1u64 << size) - 1 {
                self.cpu_error(25, &[Arg::from(val as i64), Arg::from(0i64), Arg::from((1i64 << size) - 1)]);
            }
        }
        let mut di = pos >> 3;
        let mut pos = (pos & 7) as i32;
        let mut size = size as i32;
        while size > 0 {
            let shift = 8 - pos - size;
            let v: u8 = if shift > 0 {
                ((val as u32).wrapping_shl(shift as u32) & 0xff) as u8
            } else if shift < 0 {
                ((val as u32).wrapping_shr((-shift) as u32) & 0xff) as u8
            } else {
                (val & 0xff) as u8
            };
            d[di] |= v;
            di += 1;
            size -= 8 - pos;
            pos = 0;
        }
    }

    /// write_branch(): returns new write index
    fn write_branch(&mut self, d: &mut Vec<u8>, relocs: &mut Vec<Reloc>, mut di: usize, op: &Operand, ext: u8, sec: usize, pc: Taddr, bcc: bool) -> usize {
        if !bcc && ext != b'w' && ext != b'l' {
            self.ierror(0, "encode.rs", line!());
        }
        if let Some(b) = op.base[0] {
            if self.is_pc_reloc(b, sec) {
                // external branch label, or label from a different section
                let mut addend = op.extval[0];
                let (mut size, mut offset) = (0usize, 0usize);
                match ext {
                    b'b' | b's' => {
                        addend = addend.wrapping_sub(1); // reloc offset is stored 1 byte before PC
                        d[di - 1] = (addend & 0xff) as u8;
                        size = 8;
                        offset = 1;
                    }
                    b'l' => {
                        if self.cpu.cpu_type & (m68020up | cpu32 | mcfb | mcfc | m68881 | m68851) != 0 {
                            if bcc { d[di - 1] = 0xff; }
                            offset = di;
                            d.resize(di + 4, 0);
                            setval_be(&mut d[di..], 4, addend);
                            di += 4;
                            size = 32;
                        } else {
                            self.cpu_error(0, &[]);
                        }
                    }
                    b'w' => {
                        if bcc { d[di - 1] = 0; }
                        offset = di;
                        d.resize(di + 2, 0);
                        setval_be(&mut d[di..], 2, addend);
                        di += 2;
                        size = 16;
                    }
                    _ => self.cpu_error(34, &[]),
                }
                if size != 0 {
                    self.add_extnreloc(relocs, b, addend, REL_PC, 0, size, offset);
                }
            } else {
                let diff = op.extval[0].wrapping_sub(pc);
                match ext {
                    b'b' | b's' => {
                        if diff >= -0x80 && diff <= 0x7f && diff != 0 {
                            d[di - 1] = (diff & 0xff) as u8;
                        } else {
                            self.cpu_error(28, &[]);
                        }
                    }
                    b'l' => {
                        if self.cpu.cpu_type & (m68020up | cpu32 | mcfb | mcfc | m68881 | m68851) != 0 {
                            if bcc { d[di - 1] = 0xff; }
                            d.resize(di + 4, 0);
                            setval_be(&mut d[di..], 4, diff);
                            di += 4;
                        } else {
                            self.cpu_error(0, &[]);
                        }
                    }
                    b'w' => {
                        if diff >= -0x8000 && diff <= 0x7fff {
                            if bcc { d[di - 1] = 0; }
                            d.resize(di + 2, 0);
                            setval_be(&mut d[di..], 2, diff);
                            di += 2;
                        } else {
                            self.cpu_error(28, &[]);
                        }
                    }
                    _ => self.cpu_error(34, &[]),
                }
            }
        } else {
            self.cpu_error(26, &[]);
        }
        di
    }

    /// write_extval()
    fn write_extval(num: usize, size: usize, d: &mut Vec<u8>, di: usize, op: &mut Operand, rel_abs: bool) -> usize {
        if rel_abs && op.basetype[num] == EBase::PcRel {
            op.extval[num] = op.extval[num].wrapping_add(di as Taddr);
        }
        d.resize(di + size, 0);
        setval_be(&mut d[di..], size, op.extval[num]);
        di + size
    }

    /// write_ea_ext(): returns new write index
    fn write_ea_ext(&mut self, d: &mut Vec<u8>, relocs: &mut Vec<Reloc>, mut di: usize, op: &mut Operand, ext: u8, sec: usize, pc: Taddr) -> usize {
        if op.mode > MODE_Extended || (op.mode == MODE_Extended && op.reg > REG_Immediate) {
            self.ierror(0, "encode.rs", line!());
        }
        if op.mode < MODE_An16Disp {
            return di;
        }
        let typechk = self.cpu.typechk;
        let mut rtype = REL_NONE;
        let mut roffs = di;
        let mut rsize = 0usize;
        let mut ortype = REL_NONE;
        let mut orsize = 0usize;
        if op.flags & FL_020up != 0 {
            if self.cpu.cpu_type & (m68020up | cpu32) == 0 {
                self.cpu_error(0, &[]);
            } else if op.flags & FL_noCPU32 != 0 && self.cpu.cpu_type & cpu32 != 0 {
                self.cpu_error(0, &[]);
            }
        }
        if op.mode == MODE_An16Disp {
            let mut rel_abs = false;
            let mut rel_sd = false;
            if let Some(b) = op.base[0] {
                rsize = 16;
                let extref = self.symtab.syms[b].is_extref();
                if (extref && op.reg as i32 != self.cpu.sdreg) || op.basetype[0] == EBase::PcRel {
                    rel_abs = true;
                    rtype = REL_ABS;
                } else if op.basetype[0] == EBase::Ok {
                    rel_sd = true;
                    rtype = REL_SD;
                } else {
                    self.general_error(38, &[]);
                }
            }
            if rel_sd {
                if typechk && (op.extval[0] < 0 || op.extval[0] > 0xffff) {
                    self.cpu_error(29, &[]);
                }
            } else if typechk && (op.extval[0] < -0x8000 || op.extval[0] > 0x7fff) {
                self.cpu_error(29, &[]);
            }
            di = Self::write_extval(0, 2, d, di, op, rel_abs);
        } else if op.mode == MODE_An8Format {
            if op.flags & FL_UsesFormat == 0 {
                self.ierror(0, "encode.rs", line!());
            }
            if op.format & FW_FullFormat != 0 {
                // 020+ full format: cpu_error(0) already reported above (fatal)
                d.resize(di + 2, 0);
                setval_be(&mut d[di..], 2, op.format as Taddr);
                di += 2;
            } else {
                if typechk && (op.extval[0] < -0x80 || op.extval[0] > 0x7f) {
                    self.cpu_error(29, &[]);
                }
                if let Some(b) = op.base[0] {
                    rsize = 8;
                    let s = &self.symtab.syms[b];
                    if s.is_extref() || (s.is_locref() && op.basetype[0] == EBase::PcRel) {
                        rtype = REL_ABS;
                    } else {
                        self.cpu_error(30, &[]);
                    }
                }
                d.push(((op.format >> 8) & 0xff) as u8);
                d.push((op.extval[0] & 0xff) as u8);
                di += 2;
            }
        } else if op.mode == MODE_Extended {
            if op.reg == REG_PC16Disp {
                let mut disp = op.extval[0];
                if let Some(b) = op.base[0] {
                    if self.is_pc_reloc(b, sec) {
                        rtype = REL_PC;
                        rsize = 16;
                    } else {
                        disp = op.extval[0].wrapping_sub(pc);
                    }
                }
                if typechk && (disp < -0x8000 || disp > 0x7fff) {
                    self.cpu_error(29, &[]);
                }
                d.resize(di + 2, 0);
                setval_be(&mut d[di..], 2, disp);
                di += 2;
            } else if op.reg == REG_PC8Format {
                let mut disp = op.extval[0];
                if op.flags & FL_UsesFormat == 0 {
                    self.ierror(0, "encode.rs", line!());
                }
                if op.format & FW_FullFormat != 0 {
                    d.resize(di + 2, 0);
                    setval_be(&mut d[di..], 2, op.format as Taddr);
                    di += 2;
                } else {
                    if let Some(b) = op.base[0] {
                        if self.is_pc_reloc(b, sec) {
                            rtype = REL_PC;
                            rsize = 8;
                            roffs += 1;
                            op.extval[0] = op.extval[0].wrapping_add(1); // pc-relative xref fix
                            disp = disp.wrapping_add(1);
                        } else {
                            disp = op.extval[0].wrapping_sub(pc);
                        }
                    }
                    if typechk && (disp < -0x80 || disp > 0x7f) {
                        self.cpu_error(29, &[]);
                    }
                    d.push(((op.format >> 8) & 0xff) as u8);
                    d.push((disp & 0xff) as u8);
                    di += 2;
                }
            } else if op.reg == REG_AbsShort {
                if typechk && (op.extval[0] < -0x8000 || op.extval[0] > 0x7fff) {
                    self.cpu_error(32, &[]);
                }
                let rel_abs = op.base[0].is_some();
                if rel_abs {
                    rtype = REL_ABS;
                    rsize = 16;
                }
                di = Self::write_extval(0, 2, d, di, op, rel_abs);
            } else if op.reg == REG_AbsLong {
                let rel_abs = op.base[0].is_some();
                if rel_abs {
                    rtype = REL_ABS;
                    rsize = 32;
                }
                di = Self::write_extval(0, 4, d, di, op, rel_abs);
            } else if op.reg == REG_Immediate {
                let rel_abs = op.base[0].is_some();
                if rel_abs {
                    rtype = REL_ABS;
                }
                match ext {
                    b'b' => {
                        if op.flags & FL_ExtVal0 != 0 {
                            roffs += 1;
                            rsize = 8;
                            d.push(0);
                            di += 1;
                            di = Self::write_extval(0, 1, d, di, op, rel_abs);
                            if typechk && (op.extval[0] < -0x80 || op.extval[0] > 0xff) {
                                self.cpu_error(36, &[]);
                            }
                        } else {
                            self.cpu_error(37, &[]);
                        }
                    }
                    b'w' => {
                        if op.flags & FL_ExtVal0 != 0 {
                            rsize = 16;
                            di = Self::write_extval(0, 2, d, di, op, rel_abs);
                            if typechk && (op.extval[0] < -0x8000 || op.extval[0] > 0xffff) {
                                self.cpu_error(36, &[]);
                            }
                        } else {
                            self.cpu_error(37, &[]);
                        }
                    }
                    b'l' => {
                        if op.flags & FL_ExtVal0 != 0 {
                            rsize = 32;
                            di = Self::write_extval(0, 4, d, di, op, rel_abs);
                        } else if op.value[0].as_ref().map(|v| self.type_of_expr(v)) == Some(3) {
                            let f = self.eval_expr_float(&op.value[0].clone().unwrap()).unwrap_or(0.0);
                            d.extend_from_slice(&(f as f32).to_bits().to_be_bytes());
                            di += 4;
                        } else {
                            self.cpu_error(37, &[]);
                        }
                    }
                    b's' => {
                        let f = self.float_imm(op);
                        d.extend_from_slice(&(f as f32).to_bits().to_be_bytes());
                        di += 4;
                    }
                    b'd' => {
                        let f = self.float_imm(op);
                        d.extend_from_slice(&f.to_bits().to_be_bytes());
                        di += 8;
                    }
                    b'x' | b'p' => {
                        let f = self.float_imm(op);
                        d.extend_from_slice(&conv2ieee80(f));
                        di += 12;
                    }
                    _ => {}
                }
            }
        }
        // append relocations
        if rtype != REL_NONE {
            if rtype == REL_ABS && op.basetype[0] == EBase::PcRel {
                rtype = REL_PC;
            }
            self.add_extnreloc(relocs, op.base[0].unwrap(), op.extval[0], rtype, 0, rsize, roffs);
        }
        if ortype != REL_NONE {
            if ortype == REL_ABS && op.basetype[1] == EBase::PcRel {
                ortype = REL_PC;
            }
            let o = if rtype == REL_NONE { roffs } else { roffs + rsize / 8 };
            self.add_extnreloc(relocs, op.base[1].unwrap(), op.extval[1], ortype, 0, orsize, o);
        }
        let _ = orsize;
        di
    }

    fn float_imm(&mut self, op: &Operand) -> f64 {
        match &op.value[0] {
            Some(v) => {
                let v = v.clone();
                match self.eval_expr_float(&v) {
                    Some(f) => f,
                    None => { self.cpu_error(37, &[]); 0.0 }
                }
            }
            None => 0.0,
        }
    }

    /// eval_instruction(): encode the instruction (optimising it for real)
    pub fn eval_instruction(&mut self, ip: &mut Instruction, sec: usize, pc: Taddr) -> DBlock {
        let ipflags = ip.ext.flags;
        let lastsize = ip.ext.last_size;
        self.optimize_instruction_final(ip, sec, pc);
        let size = Self::iplist_size(ip);
        let mut d: Vec<u8> = Vec::with_capacity(size);
        let mut relocs: Vec<Reloc> = Vec::new();
        if size == 0 {
            ip.ext.flags = ipflags;
            ip.ext.last_size = lastsize;
            return DBlock::new(d);
        }
        let mut pc = pc;
        let mut cur: Option<&mut Instruction> = Some(ip);
        while let Some(i) = cur {
            if i.code >= 0 {
                let mnemo = MNEMONICS[i.code as usize];
                let ext = i.qual;
                let sz = if mnemo.size & SIZE_MASK == SIZE_UNSIZED { SIZE_UNSIZED } else { lc_ext_to_size(ext) };
                let dbstart = d.len();
                if mnemo.available & malias != 0 {
                    self.cpu_error(33, &[]);
                }
                d.push((mnemo.opcode[0] >> 8) as u8);
                d.push((mnemo.opcode[0] & 0xff) as u8);
                pc = pc.wrapping_add(2);
                if mnemo.size & 3 > 1 {
                    d.push((mnemo.opcode[1] >> 8) as u8);
                    d.push((mnemo.opcode[1] & 0xff) as u8);
                    pc = pc.wrapping_add(2);
                    if mnemo.size & 3 > 2 {
                        d.push(0);
                        d.push(0);
                        pc = pc.wrapping_add(2);
                    }
                }
                match mnemo.size & 0x7c {
                    S_NONE => {}
                    S_STD => match sz { SIZE_WORD => d[dbstart + 1] |= 0x40, SIZE_LONG => d[dbstart + 1] |= 0x80, _ => {} },
                    S_STD1 => match sz { SIZE_BYTE => d[dbstart + 1] |= 0x40, SIZE_WORD => d[dbstart + 1] |= 0x80, SIZE_LONG => d[dbstart + 1] |= 0xc0, _ => {} },
                    S_HI => match sz { SIZE_WORD => d[dbstart] |= 0x02, SIZE_LONG => d[dbstart] |= 0x04, _ => {} },
                    S_CAS => match sz { SIZE_BYTE => d[dbstart] |= 0x02, SIZE_WORD => d[dbstart] |= 0x04, SIZE_LONG => d[dbstart] |= 0x06, _ => {} },
                    S_MOVE => match sz { SIZE_BYTE => d[dbstart] |= 0x10, SIZE_WORD => d[dbstart] |= 0x30, SIZE_LONG => d[dbstart] |= 0x20, _ => {} },
                    S_WL8 => { if sz == SIZE_LONG { d[dbstart] |= 1; } }
                    S_LW7 => { if sz == SIZE_WORD { d[dbstart + 1] |= 0x80; } }
                    S_WL6 => { if sz == SIZE_LONG { d[dbstart + 1] |= 0x40; } }
                    S_MAC => { if sz == SIZE_LONG { d[dbstart + 2] |= 8; } }
                    S_TRAP => match sz { SIZE_WORD => d[dbstart + 1] |= 0x02, SIZE_LONG => d[dbstart + 1] |= 0x03, _ => {} },
                    S_EXT => match sz { SIZE_WORD => d[dbstart + 3] |= 0x40, SIZE_LONG => d[dbstart + 3] |= 0x80, _ => {} },
                    S_FP => match sz {
                        SIZE_SINGLE => d[dbstart + 2] |= 0x04, SIZE_EXTENDED => d[dbstart + 2] |= 0x08,
                        SIZE_PACKED => d[dbstart + 2] |= 0x0c, SIZE_WORD => d[dbstart + 2] |= 0x10,
                        SIZE_DOUBLE => d[dbstart + 2] |= 0x14, SIZE_BYTE => d[dbstart + 2] |= 0x18, _ => {}
                    },
                    _ => self.ierror(0, "encode.rs", line!()),
                }
                for k in 0..MAX_OPERANDS {
                    let op = match i.op[k].as_mut() {
                        Some(o) => o,
                        None => break,
                    };
                    let oii = INSERT_INFO[mnemo.place[k] as usize];
                    let mut newd = d.len();
                    match oii.mode {
                        M_ea | M_noea | M_nop | M_bfea => {
                            if oii.mode == M_ea || oii.mode == M_bfea {
                                if oii.flags & IIF_MASK != 0 {
                                    d[dbstart + 3] |= if op.bf_width != 0 { 1 << oii.pos } else { 0 };
                                }
                                if oii.flags & IIF_NOMODE != 0 {
                                    d[dbstart + 1] |= (op.reg & 7) as u8;
                                } else {
                                    d[dbstart + 1] |= (((op.mode & 7) << 3) | (op.reg & 7)) as u8;
                                }
                            }
                            if oii.mode != M_nop {
                                let mut o2 = std::mem::replace(op, Operand::new());
                                newd = self.write_ea_ext(&mut d, &mut relocs, newd, &mut o2, ext, sec, pc);
                                *op = o2;
                            }
                        }
                        M_high_ea => {
                            d[dbstart] |= (((op.reg & 7) << 1) | ((op.mode & 4) >> 2)) as u8;
                            d[dbstart + 1] |= ((op.mode & 3) << 6) as u8;
                            let mut o2 = std::mem::replace(op, Operand::new());
                            newd = self.write_ea_ext(&mut d, &mut relocs, newd, &mut o2, ext, sec, pc);
                            *op = o2;
                        }
                        M_branch => {
                            let o2 = op.clone();
                            newd = self.write_branch(&mut d, &mut relocs, newd, &o2, ext, sec, pc, oii.flags & IIF_BCC != 0);
                        }
                        M_val0 => {
                            if op.base[0].is_none() {
                                let mut v = op.extval[0];
                                if oii.flags & IIF_MASK != 0 {
                                    if v == 0 { v = 1 << oii.size; } else if v == (1 << oii.size) { v = 0; }
                                } else if oii.flags & IIF_3Q != 0 {
                                    if v == 0 { v = -1; } else if v == -1 { v = 0; }
                                }
                                if oii.flags & IIF_REVERSE != 0 {
                                    v = reverse(v as u32, oii.size as u32);
                                }
                                let (pos, size, signed) = (oii.pos as usize, oii.size as u32, oii.flags & IIF_SIGNED != 0);
                                let mut buf = std::mem::take(&mut d);
                                self.write_val(&mut buf[dbstart..], pos, size, v, signed);
                                d = buf;
                            } else {
                                self.cpu_error(24, &[]);
                            }
                        }
                        M_reg => {
                            if op.mode < MODE_Extended || op.mode == MODE_FPn {
                                let mut r = op.reg as Taddr;
                                if oii.size > 3 && op.mode == MODE_An {
                                    r += 8;
                                }
                                let (pos, size) = (oii.pos as usize, oii.size as u32);
                                let mut buf = std::mem::take(&mut d);
                                self.write_val(&mut buf[dbstart..], pos, size, r, false);
                                d = buf;
                            } else {
                                self.ierror(0, "encode.rs", line!());
                            }
                        }
                        M_func => {
                            // 020+/CF/FPU-only insertion functions; unreachable on 68000
                            self.cpu_error(0, &[]);
                        }
                        _ => self.ierror(0, "encode.rs", line!()),
                    }
                    pc = pc.wrapping_add((newd - d.len().min(newd)) as Taddr);
                    // (d has already grown to newd)
                    let _ = newd;
                }
            }
            cur = i.next.as_deref_mut();
        }
        ip.ext.flags = ipflags;
        ip.ext.last_size = lastsize;
        DBlock { data: d, relocs }
    }

    fn optimize_instruction_final(&mut self, ip: &mut Instruction, sec: usize, pc: Taddr) {
        self.optimize_instruction_pub(ip, sec, pc, true);
    }

    /// eval_data(): dc.X operand to bytes
    pub fn eval_data(&mut self, op: &mut Operand, bitsize: i32, sec: usize, pc: Taddr) -> DBlock {
        let size = (bitsize >> 3) as usize;
        let mut data = vec![0u8; size];
        let v = op.value[0].clone().unwrap();
        let etype = self.type_of_expr(&v);
        let mut val: Taddr = 0;
        let mut hval: Thuge = 0;
        let mut fval: f64 = 0.0;
        let mut base: Option<usize> = None;
        let mut btype = EBase::None;
        let etype = if etype == 3 {
            match self.eval_expr_float(&v) {
                Some(f) => fval = f,
                None => self.general_error(60, &[]),
            }
            3
        } else if bitsize > 32 {
            match self.eval_expr_huge(&v) {
                Some(h) => hval = h,
                None => self.general_error(59, &[]),
            }
            2
        } else {
            let (vv, cnst) = self.eval_expr(&v, Some(sec), pc);
            val = vv;
            if !cnst {
                let (bt, b) = self.find_base(&v, Some(sec), pc);
                if bt == EBase::Illegal {
                    self.general_error(38, &[]);
                }
                base = b;
                btype = bt;
            }
            1
        };
        let typechk = self.cpu.typechk;
        match bitsize {
            8 => {
                if etype == 1 {
                    if typechk && (val < -0x80 || val > 0xff) {
                        self.cpu_error(39, &[]);
                    }
                    data[0] = (val & 0xff) as u8;
                } else if etype == 3 {
                    self.cpu_error(40, &[]);
                }
            }
            16 => {
                if etype == 1 {
                    if typechk && (val < -0x8000 || val > 0xffff) {
                        self.cpu_error(39, &[]);
                    }
                    setval_be(&mut data, 2, val);
                } else if etype == 3 {
                    self.cpu_error(40, &[]);
                }
            }
            32 => {
                if etype == 1 {
                    setval_be(&mut data, 4, val);
                } else if etype == 3 {
                    data.copy_from_slice(&(fval as f32).to_bits().to_be_bytes());
                }
            }
            64 => {
                if etype == 2 {
                    if typechk && !huge_chkrange(hval, 64) {
                        self.cpu_error(39, &[]);
                    }
                    data.copy_from_slice(&(hval as u64).to_be_bytes());
                } else if etype == 3 {
                    data.copy_from_slice(&fval.to_bits().to_be_bytes());
                }
            }
            96 => {
                if etype == 2 {
                    if typechk && !huge_chkrange(hval, 96) {
                        self.cpu_error(39, &[]);
                    }
                    let b = (hval as u128).to_be_bytes();
                    data.copy_from_slice(&b[4..16]);
                } else if etype == 3 {
                    data.copy_from_slice(&conv2ieee80(fval));
                }
            }
            _ => self.cpu_error(38, &[Arg::from(bitsize)]),
        }
        let mut relocs: Vec<Reloc> = Vec::new();
        if let Some(b) = base {
            // relocation required
            self.add_extnreloc(&mut relocs, b, val, if btype == EBase::PcRel { REL_PC } else { REL_ABS }, 0, bitsize as usize, 0);
        }
        DBlock { data, relocs }
    }
}

/// conv2ieee80(): 96-bit extended precision (sign/exponent word, 16 zero bits, 64-bit mantissa)
pub fn conv2ieee80(f: f64) -> [u8; 12] {
    let mut out = [0u8; 12];
    if f == 0.0 {
        if f.is_sign_negative() {
            out[0] = 0x80;
        }
        return out;
    }
    let bits = f.to_bits();
    let sign = (bits >> 63) as u16;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let man = bits & 0xf_ffff_ffff_ffff;
    let (e, m): (u16, u64) = if exp == 0x7ff {
        (0x7fff, if man == 0 { 0x8000_0000_0000_0000 } else { (man << 11) | 0x8000_0000_0000_0000 })
    } else if exp == 0 {
        // denormal double: normalise
        let mut m = man;
        let mut e = -1022i32;
        while m & (1 << 52) == 0 {
            m <<= 1;
            e -= 1;
        }
        ((e + 16383) as u16, m << 11)
    } else {
        ((exp - 1023 + 16383) as u16, (man << 11) | 0x8000_0000_0000_0000)
    };
    let se = (sign << 15) | e;
    out[0..2].copy_from_slice(&se.to_be_bytes());
    out[4..12].copy_from_slice(&m.to_be_bytes());
    out
}
