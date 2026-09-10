//! Central assembler state: vasm's file-scope globals gathered in one struct.
use crate::atoms::Section;
use crate::cli::Options;
use crate::errors::ErrorState;
use crate::expr::Expr;
use crate::m68k::CpuState;
use crate::source::{Macro, Source};
use crate::symbols::SymTab;
use std::collections::HashMap;

pub const MAXCONDLEV: usize = 63;
pub const IDSTACKSIZE: usize = 100;
pub const INLSTACKSIZE: usize = 100;

/// cond.c state
pub struct CondState {
    pub cond: [bool; MAXCONDLEV + 1],
    pub condsrc: [String; MAXCONDLEV + 1],
    pub condline: [i32; MAXCONDLEV + 1],
    pub clev: usize,
    pub ifnesting: i32,
}

impl CondState {
    pub fn new() -> Self {
        let mut c = CondState {
            cond: [false; MAXCONDLEV + 1],
            condsrc: std::array::from_fn(|_| String::new()),
            condline: [0; MAXCONDLEV + 1],
            clev: 0,
            ifnesting: 0,
        };
        c.cond[0] = true;
        c
    }
}

/// Register symbol created by EQUR (cpu.c new_regsym)
#[derive(Debug, Clone)]
pub struct RegSym {
    pub rtype: u8,
    pub reg: i32,
}

pub struct Assembler {
    pub opts: Options,
    pub errs: ErrorState,
    pub symtab: SymTab,
    pub sections: Vec<Section>,
    pub current_section: Option<usize>,
    pub offset_id: u64,
    pub cpc: Option<usize>,
    pub unsigned_shift: bool,
    pub final_pass: bool,
    /// current line buffer: [0, text..., 0]
    pub line: Vec<u8>,
    pub sources: Vec<Source>,
    pub cur_src: Option<usize>,
    pub source_id: u64,
    pub include_paths: Vec<String>,
    pub included: Vec<String>,
    pub macros: Vec<Macro>,
    pub macro_map: HashMap<String, usize>,
    pub cur_macro: Option<usize>,
    pub enddir_list: Option<&'static [&'static [u8]]>,
    pub reptdir_list: Option<&'static [&'static [u8]]>,
    pub rept_cnt: i32,
    pub rept_start: usize,
    pub maxmacparams: usize,
    pub maxmacrecurs: u32,
    pub nocase_macros: bool,
    pub esc_sequences: bool,
    pub cond: CondState,
    pub cpu: CpuState,
    pub parse_end: bool,
    pub id_stack: Vec<u64>,
    pub inline_stack: Vec<u64>,
    pub inline_id: u64,
    pub saved_last_global_label: Option<String>,
    pub regsyms: HashMap<String, RegSym>,
    pub baseexp: [Option<Expr>; 7],
    pub align_data: bool,
    pub check_comm: bool,
    pub warn_unalloc_ini_dat: bool,
    pub auto_import: bool,
    pub ignore_multinc: bool,
    pub filename: Option<String>,
    pub secname_attr: bool,
    /// symbol-table save point for new_inst's save_symbols()/restore_symbols()
    pub saved_symcount: usize,
    pub last_sdreg: i32,
}

impl Assembler {
    pub fn new(opts: Options) -> Self {
        let mut errs = ErrorState::default();
        errs.max_errors = opts.max_errors;
        errs.no_warn = opts.warnings_off;
        errs.disabled = opts.no_warn.clone();
        let mut a = Assembler {
            symtab: SymTab { nocase: opts.nocase, ..Default::default() },
            errs,
            sections: Vec::new(),
            current_section: None,
            offset_id: 0,
            cpc: None,
            unsigned_shift: opts.unsigned_shift,
            final_pass: false,
            line: vec![0, 0, 0, 0],
            sources: Vec::new(),
            cur_src: None,
            source_id: 0,
            include_paths: vec!["./".to_string()],
            included: Vec::new(),
            macros: Vec::new(),
            macro_map: HashMap::new(),
            cur_macro: None,
            enddir_list: None,
            reptdir_list: None,
            rept_cnt: -1,
            rept_start: 0,
            maxmacparams: if opts.allmp { 35 } else { 9 },
            maxmacrecurs: 1000,
            nocase_macros: false,
            esc_sequences: opts.esc_sequences,
            cond: CondState::new(),
            cpu: CpuState::new(),
            parse_end: false,
            id_stack: Vec::new(),
            inline_stack: Vec::new(),
            inline_id: 0,
            saved_last_global_label: None,
            regsyms: HashMap::new(),
            baseexp: Default::default(),
            align_data: opts.align_data,
            check_comm: false,
            warn_unalloc_ini_dat: false,
            auto_import: !opts.exit_on_first_error,
            ignore_multinc: opts.ignore_mult_inc,
            filename: None,
            secname_attr: true,
            saved_symcount: 0,
            last_sdreg: -1,
            opts,
        };
        // init_cpu(): predefine register symbols sp and fp
        a.regsyms.insert("sp".to_string(), RegSym { rtype: 1, reg: 7 });
        a.regsyms.insert("fp".to_string(), RegSym { rtype: 1, reg: 6 });
        // -no-opt
        if a.opts.no_opt {
            a.cpu.clear_all_opts();
            a.cpu.no_opt = true;
        }
        let o = &a.opts;
        let no = a.cpu.no_opt;
        if o.opt_movem { a.cpu.opt_movem = !no; }
        if o.opt_pea { a.cpu.opt_pea = !no; }
        if o.opt_clr { a.cpu.opt_clr = !no; }
        if o.opt_st { a.cpu.opt_st = !no; }
        if o.opt_lsl { a.cpu.opt_lsl = !no; }
        if o.opt_mul { a.cpu.opt_mul = !no; }
        if o.opt_div { a.cpu.opt_div = !no; }
        if o.opt_fconst { a.cpu.opt_fconst = !no; }
        if o.opt_brajmp { a.cpu.opt_brajmp = !no; }
        if o.opt_allbra { a.cpu.opt_allbra = !no; }
        if o.opt_speed { a.cpu.opt_speed = !no; }
        if o.show_opt { a.cpu.warn_opts = 2; }
        if o.range_warnings { a.errs.range_warnings = true; }
        a
    }

    /// save_symbols(): remember the symbol count
    pub fn save_symbols(&mut self) {
        self.saved_symcount = self.symtab.syms.len();
    }
    /// restore_symbols(): drop symbols created since save_symbols()
    pub fn restore_symbols(&mut self) {
        let n = self.saved_symcount;
        if self.symtab.syms.len() > n {
            for i in n..self.symtab.syms.len() {
                let name = self.symtab.syms[i].name.clone();
                let key = if self.symtab.nocase { name.to_ascii_lowercase() } else { name };
                if self.symtab.map.get(&key) == Some(&i) {
                    self.symtab.map.remove(&key);
                }
            }
            self.symtab.syms.truncate(n);
        }
    }
}
