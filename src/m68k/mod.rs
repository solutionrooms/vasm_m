//! m68k backend: operand/instruction types (cpu.h), option state (cpu.c
//! opt_* globals), and the entry points used by the core.
pub mod tables;
pub mod parse;
pub mod optimize;
pub mod encode;

use crate::expr::Expr;
use crate::types::Taddr;
use std::rc::Rc;

pub const MODE_Dn: i8 = 0;
pub const MODE_An: i8 = 1;
pub const MODE_AnIndir: i8 = 2;
pub const MODE_AnPostInc: i8 = 3;
pub const MODE_AnPreDec: i8 = 4;
pub const MODE_An16Disp: i8 = 5;
pub const MODE_An8Format: i8 = 6;
pub const MODE_Extended: i8 = 7;
pub const MODE_FPn: i8 = 8;
pub const MODE_SpecReg: i8 = 9;
pub const REG_AbsShort: i8 = 0;
pub const REG_AbsLong: i8 = 1;
pub const REG_PC16Disp: i8 = 2;
pub const REG_PC8Format: i8 = 3;
pub const REG_Immediate: i8 = 4;
pub const REG_RnList: i8 = 5;
pub const REG_FPnList: i8 = 6;

pub const FL_ExtVal0: u32 = 1;
pub const FL_ExtVal1: u32 = 2;
pub const FL_UsesFormat: u32 = 4;
pub const FL_020up: u32 = 8;
pub const FL_noCPU32: u32 = 0x10;
pub const FL_NoOptBase: u32 = 0x100;
pub const FL_NoOptOuter: u32 = 0x200;
pub const FL_NoOpt: u32 = 0x300;
pub const FL_DoNotEval: u32 = 0x400;
pub const FL_CheckMask: u32 = 0xf800;
pub const FL_MAC: u32 = 0x800;
pub const FL_Bitfield: u32 = 0x1000;
pub const FL_DoubleReg: u32 = 0x2000;
pub const FL_KFactor: u32 = 0x4000;
pub const FL_FPSpec: u32 = 0x8000;
pub const FL_BaseReg: u32 = 0x10000;

pub const FW_IndexAn: u16 = 0x8000;
pub const FW_LongIndex: u16 = 0x0800;
pub const FW_FullFormat: u16 = 0x0100;
pub const FW_BaseSuppress: u16 = 0x0080;
pub const FW_IndexSuppress: u16 = 0x0040;
#[inline] pub fn fw_index_reg(n: u16) -> u16 { n << 12 }
#[inline] pub fn fw_scale(n: u16) -> u16 { n << 9 }
pub const FW_BDSize_Mask: u16 = 0x0030;
pub const FW_IndSize_Mask: u16 = 0x0003;

pub const EXT_NONE: i32 = 0;
pub const EXT_BYTE: i32 = 1;
pub const EXT_WORD: i32 = 2;
pub const EXT_LONG: i32 = 3;
pub const EXT_SINGLE: i32 = 4;
pub const EXT_DOUBLE: i32 = 5;
pub const EXT_EXTENDED: i32 = 6;
pub const EXT_PACKED: i32 = 7;

pub const IFL_RETAINLASTSIZE: u8 = 1;
pub const IFL_UNSIZED: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Illegal = 0,
    Ok = 1,
    PcRel = 2,
    None = -1,
}

/// cpu.h `operand`
#[derive(Debug, Clone)]
pub struct Operand {
    pub mode: i8,
    pub reg: i8,
    pub format: u16,
    pub bf_offset: u8,
    pub bf_width: u8,
    pub basetype: [crate::expr::Base; 2],
    pub flags: u32,
    pub value: [Option<Rc<Expr>>; 2],
    pub extval: [Taddr; 2],
    pub base: [Option<usize>; 2],
}

impl Operand {
    pub fn new() -> Self {
        Operand {
            mode: -1,
            reg: -1,
            format: 0,
            bf_offset: 0,
            bf_width: 0,
            basetype: [crate::expr::Base::None; 2],
            flags: 0,
            value: [None, None],
            extval: [0, 0],
            base: [None, None],
        }
    }
}

/// cpu.h `instruction_ext.un.real`
#[derive(Debug, Clone, Copy)]
pub struct InstExt {
    pub flags: u8,
    pub last_size: i8,
    pub orig_ext: i8,
}

/// vasm.h `instruction` with the m68k extension and the copy chain
#[derive(Debug, Clone)]
pub struct Instruction {
    pub code: i32,
    /// qualifier[0] as lowercase first char (0 = none). vasm keeps a string;
    /// only its first character is ever compared.
    pub qual: u8,
    pub op: [Option<Operand>; 6],
    pub ext: InstExt,
    pub next: Option<Box<Instruction>>,
}

impl Instruction {
    pub fn new(code: i32) -> Self {
        Instruction {
            code,
            qual: 0,
            op: [None, None, None, None, None, None],
            ext: InstExt { flags: 0, last_size: -1, orig_ext: -1 },
            next: None,
        }
    }
}

/// cpu.c opt_* globals and other backend state
#[derive(Debug, Clone)]
pub struct CpuState {
    pub cpu_type: u32,
    pub opt_gen: bool,
    pub opt_movem: bool,
    pub opt_pea: bool,
    pub opt_clr: bool,
    pub opt_st: bool,
    pub opt_lsl: bool,
    pub opt_mul: bool,
    pub opt_div: bool,
    pub opt_fconst: bool,
    pub opt_brajmp: bool,
    pub opt_pc: bool,
    pub opt_bra: bool,
    pub opt_allbra: bool,
    pub opt_disp: bool,
    pub opt_abs: bool,
    pub opt_moveq: bool,
    pub opt_quick: bool,
    pub opt_branop: bool,
    pub opt_bdisp: bool,
    pub opt_odisp: bool,
    pub opt_lea: bool,
    pub opt_lquick: bool,
    pub opt_immaddr: bool,
    pub opt_speed: bool,
    pub opt_sc: bool,
    pub no_opt: bool,
    pub warn_opts: u8,
    pub typechk: bool,
    pub ign_unambig_ext: bool,
    pub sdreg: i32,
    pub elfregs: bool,
    pub sgs: bool,
    pub devpac_compat: bool,
    pub phxass_compat: bool,
    pub current_ext: u8,
    /// indices resolved from code_tab
    pub oc: OpCodes,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpCodes {
    pub jmp: i32, pub jsr: i32, pub moveq: i32, pub mov3q: i32, pub lea: i32, pub pea: i32,
    pub suba: i32, pub clr: i32, pub st: i32, pub addq: i32, pub subq: i32, pub adda: i32,
    pub add: i32, pub bra: i32, pub bsr: i32, pub tst: i32, pub not: i32, pub noop: i32,
    pub fnop: i32, pub movea: i32, pub ext: i32, pub mvz: i32, pub mov: i32, pub asri: i32,
    pub lsri: i32, pub asli: i32, pub lsli: i32, pub neg: i32,
}

impl CpuState {
    pub fn new() -> Self {
        let mut s = CpuState {
            cpu_type: tables::m68000,
            opt_gen: true,
            opt_movem: false,
            opt_pea: false,
            opt_clr: false,
            opt_st: false,
            opt_lsl: false,
            opt_mul: false,
            opt_div: false,
            opt_fconst: true,
            opt_brajmp: false,
            opt_pc: true,
            opt_bra: true,
            opt_allbra: false,
            opt_disp: true,
            opt_abs: true,
            opt_moveq: true,
            opt_quick: true,
            opt_branop: true,
            opt_bdisp: true,
            opt_odisp: true,
            opt_lea: true,
            opt_lquick: true,
            opt_immaddr: true,
            opt_speed: false,
            opt_sc: false,
            no_opt: false,
            warn_opts: 0,
            typechk: true,
            ign_unambig_ext: false,
            sdreg: -1,
            elfregs: false,
            sgs: false,
            devpac_compat: false,
            phxass_compat: false,
            current_ext: 0,
            oc: OpCodes::default(),
        };
        s.oc = resolve_code_tab();
        s
    }

    /// clear_all_opts()
    pub fn clear_all_opts(&mut self) {
        self.opt_gen = false; self.opt_movem = false; self.opt_pea = false; self.opt_clr = false;
        self.opt_st = false; self.opt_lsl = false; self.opt_mul = false; self.opt_div = false;
        self.opt_fconst = false; self.opt_brajmp = false; self.opt_pc = false; self.opt_bra = false;
        self.opt_allbra = false; self.opt_disp = false; self.opt_abs = false; self.opt_moveq = false;
        self.opt_quick = false; self.opt_branop = false; self.opt_bdisp = false; self.opt_odisp = false;
        self.opt_lea = false; self.opt_lquick = false; self.opt_immaddr = false; self.opt_speed = false;
        self.opt_sc = false;
    }
}

/// init_cpu(): resolve code_tab entries to mnemonic indices, in table order.
fn resolve_code_tab() -> OpCodes {
    use tables::*;
    // (name, optype0, optype1) in the exact order of cpu.c code_tab[]
    let tab: [(&str, u8, u8); 35] = [
        ("add", DA, 0), ("adda", 0, 0), ("addq", 0, AD), ("asl", QI, 0), ("asr", QI, 0),
        ("bra", 0, 0), ("bsr", 0, 0), ("clr", 0, 0), ("ext", 0, 0),
        ("fmovem", MR, FL), ("fmovem", FS, AM), ("fmovem", MA, FS), ("fmul", FA, F_),
        ("fsmul", FA, F_), ("fdmul", FA, F_), ("fnop", 0, 0), ("fsglmul", FA, F_),
        ("jmp", 0, 0), ("jsr", 0, 0), ("lea", 0, 0), ("lsl", QI, 0), ("lsr", QI, 0),
        ("mov3q", 0, 0), ("move", DA, AD), ("movea", 0, 0), ("moveq", 0, 0), ("mvz", 0, 0),
        ("neg", D_, 0), ("not", 0, 0), ("pea", 0, 0), ("st", AD, 0), ("suba", 0, 0),
        ("subq", 0, AD), ("tst", 0, 0), (" no-op", 0, 0),
    ];
    let mut found = [-1i32; 35];
    let mut j = 0;
    for (i, m) in MNEMONICS.iter().enumerate() {
        if j >= tab.len() { break; }
        if m.name == tab[j].0
            && (tab[j].1 == 0 || m.operand_type[0] == tab[j].1)
            && (tab[j].2 == 0 || m.operand_type[1] == tab[j].2)
        {
            found[j] = i as i32;
            j += 1;
        }
    }
    assert!(j == tab.len(), "code_tab resolution failed at {}", j);
    OpCodes {
        add: found[0], adda: found[1], addq: found[2], asli: found[3], asri: found[4],
        bra: found[5], bsr: found[6], clr: found[7], ext: found[8],
        fnop: found[15], jmp: found[17], jsr: found[18], lea: found[19], lsli: found[20],
        lsri: found[21], mov3q: found[22], mov: found[23], movea: found[24], moveq: found[25],
        mvz: found[26], neg: found[27], not: found[28], pea: found[29], st: found[30],
        suba: found[31], subq: found[32], tst: found[33], noop: found[34],
    }
}

/// SIZE_UNAMBIG flag computed as in init_cpu(): size mask has exactly one bit
#[inline]
pub fn mn_size(code: i32) -> u16 {
    let s = tables::MNEMONICS[code as usize].size;
    if (s & tables::SIZE_MASK).count_ones() == 1 { s | tables::SIZE_UNAMBIG } else { s }
}

/// lc_ext_to_size()
pub fn lc_ext_to_size(ext: u8) -> u16 {
    use tables::*;
    match ext {
        b'b' => SIZE_BYTE,
        b'w' => SIZE_WORD,
        b'l' => SIZE_LONG,
        b's' => SIZE_SINGLE,
        b'd' => SIZE_DOUBLE,
        b'x' => SIZE_EXTENDED,
        b'p' => SIZE_PACKED,
        _ => SIZE_UNSIZED,
    }
}

/// OPTS atom payload: cpu_opts() commands (add_cpu_opt)
#[derive(Debug, Clone, Copy)]
pub struct CpuOpt {
    pub cmd: u8,
    pub arg: i32,
}
