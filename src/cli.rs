//! Command-line options, mirroring vasm 1.7h's option parsing
//! (vasm.c, cpus/m68k/cpu.c cpu_args(), syntax/mot/syntax.c syntax_args()).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Bin,
    Hunk,
    HunkExe,
    Test,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub input: Option<String>,
    pub output: Option<String>,
    pub format: OutputFormat,
    pub quiet: bool,
    pub no_symbols: bool,
    pub include_paths: Vec<String>,
    /// `-Dsym[=val]` definitions, in order.
    pub defines: Vec<(String, i64)>,
    pub listing: Option<String>,
    pub max_errors: u32,
    pub no_warn: Vec<u32>,
    pub warnings_off: bool,
    pub exit_on_first_error: bool,
    /// `-spaces`: allow spaces inside operands (mot syntax).
    pub allow_spaces: bool,
    /// `-align`: natural alignment for data directives (mot syntax).
    pub align_data: bool,
    /// `-ldots`, `-localu`
    pub local_dots: bool,
    pub local_u: bool,
    /// `-no-opt`
    pub no_opt: bool,
    /// Individual `-opt-*` switches (cpu.c).
    pub opt_movem: bool,
    pub opt_pea: bool,
    pub opt_clr: bool,
    pub opt_st: bool,
    pub opt_lsl: bool,
    pub opt_mul: bool,
    pub opt_div: bool,
    pub opt_fconst: bool,
    pub opt_brajmp: bool,
    pub opt_allbra: bool,
    pub opt_speed: bool,
    pub show_opt: bool,
    pub range_warnings: bool,
    pub nocase: bool,
    pub unsigned_shift: bool,
    pub allmp: bool,
    pub esc_sequences: bool,
    pub ignore_mult_inc: bool,
    pub chklabels: bool,
    pub regsymredef: bool,
    pub threads: usize,
    /// hunk output module options (output_hunk.c)
    pub hunk_kick1: bool,
    pub hunk_linedebug: bool,
    pub hunk_keepempty: bool,
    pub hunk_databss: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input: None,
            output: None,
            format: OutputFormat::Test,
            quiet: false,
            no_symbols: false,
            include_paths: Vec::new(),
            defines: Vec::new(),
            listing: None,
            max_errors: 5,
            no_warn: Vec::new(),
            warnings_off: false,
            exit_on_first_error: false,
            allow_spaces: false,
            align_data: false,
            local_dots: false,
            local_u: false,
            no_opt: false,
            opt_movem: false,
            opt_pea: false,
            opt_clr: false,
            opt_st: false,
            opt_lsl: false,
            opt_mul: false,
            opt_div: false,
            opt_fconst: false,
            opt_brajmp: false,
            opt_allbra: false,
            opt_speed: false,
            show_opt: false,
            range_warnings: false,
            nocase: false,
            unsigned_shift: false,
            allmp: false,
            esc_sequences: false,
            ignore_mult_inc: false,
            chklabels: false,
            regsymredef: false,
            threads: 0,
            hunk_kick1: false,
            hunk_linedebug: false,
            hunk_keepempty: false,
            hunk_databss: false,
        }
    }
}

pub fn usage() -> String {
    "vasm_m: multithreaded 68000 assembler, vasm 1.7h compatible\n\
     usage: vasm_m [-Fbin|-Fhunk|-Fhunkexe] [-o out] [-quiet] [-spaces] [-m68000] [-Ipath] [-Dsym[=val]] \
     [-no-opt] [-opt-*] [-nosym] [-maxerrors=n] [-x] [-w] [-threads=n] \
     [-kick1hunks] [-linedebug] [-keepempty] [-databss] file.s\n"
        .to_string()
}

pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    // vasm picks the output module (-F) before parsing the other options, so
    // module-specific options are accepted regardless of their position.
    for a in args {
        if let Some(f) = a.strip_prefix("-F") {
            o.format = match f {
                "bin" => OutputFormat::Bin,
                "hunk" => OutputFormat::Hunk,
                "hunkexe" => OutputFormat::HunkExe,
                "test" => OutputFormat::Test,
                other => return Err(format!("unsupported output format: {}", other)),
            };
        }
    }
    let is_hunk = matches!(o.format, OutputFormat::Hunk | OutputFormat::HunkExe);
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if !a.starts_with('-') || a == "-" {
            if o.input.is_some() {
                return Err(format!("more than one input file: {}", a));
            }
            o.input = Some(a.to_string());
            i += 1;
            continue;
        }
        match a {
            "-quiet" => o.quiet = true,
            "-nosym" => o.no_symbols = true,
            "-x" => o.exit_on_first_error = true,
            "-w" => o.warnings_off = true,
            "-spaces" => o.allow_spaces = true,
            "-align" => o.align_data = true,
            "-ldots" => o.local_dots = true,
            "-localu" => o.local_u = true,
            "-no-opt" => o.no_opt = true,
            "-opt-movem" => o.opt_movem = true,
            "-opt-pea" => o.opt_pea = true,
            "-opt-clr" => o.opt_clr = true,
            "-opt-st" => o.opt_st = true,
            "-opt-lsl" => o.opt_lsl = true,
            "-opt-mul" => o.opt_mul = true,
            "-opt-div" => o.opt_div = true,
            "-opt-fconst" => o.opt_fconst = true,
            "-opt-brajmp" => o.opt_brajmp = true,
            "-opt-allbra" => o.opt_allbra = true,
            "-opt-speed" => o.opt_speed = true,
            "-showopt" => o.show_opt = true,
            "-rangewarnings" => o.range_warnings = true,
            "-nocase" => o.nocase = true,
            "-unsshift" => o.unsigned_shift = true,
            "-allmp" => o.allmp = true,
            "-esc" => o.esc_sequences = true,
            "-noesc" => o.esc_sequences = false,
            "-ignore-mult-inc" => o.ignore_mult_inc = true,
            "-chklabels" => o.chklabels = true,
            "-regsymredef" => o.regsymredef = true,
            "-no-fpu" | "-unnamed-sections" | "-noialign" | "-pic" | "-warncomm" => {}
            "-kick1hunks" if is_hunk => o.hunk_kick1 = true,
            "-linedebug" if is_hunk => o.hunk_linedebug = true,
            "-keepempty" if is_hunk => o.hunk_keepempty = true,
            "-databss" if o.format == OutputFormat::HunkExe => o.hunk_databss = true,
            "-o" => {
                i += 1;
                o.output = Some(args.get(i).ok_or("missing filename after -o")?.clone());
            }
            "-L" => {
                i += 1;
                o.listing = Some(args.get(i).ok_or("missing filename after -L")?.clone());
            }
            "-m68000" | "-m68k" | "-m68008" => {}
            "-devpac" | "-phxass" | "-gas" | "-sgs" | "-sc" | "-elfregs" | "-conv-brackets" => {
                return Err(format!("option not supported by vasm_m: {}", a));
            }
            _ => {
                if a.starts_with("-F") {
                    // handled by the pre-scan above
                } else if let Some(p) = a.strip_prefix("-I") {
                    o.include_paths.push(p.to_string());
                } else if let Some(d) = a.strip_prefix("-D") {
                    let (name, val) = match d.split_once('=') {
                        Some((n, v)) => (n.to_string(), parse_int(v)?),
                        None => (d.to_string(), 1),
                    };
                    o.defines.push((name, val));
                } else if let Some(n) = a.strip_prefix("-maxerrors=") {
                    o.max_errors = n.parse().map_err(|_| format!("bad -maxerrors value: {}", n))?;
                } else if let Some(n) = a.strip_prefix("-nowarn=") {
                    o.no_warn.push(n.parse().map_err(|_| format!("bad -nowarn value: {}", n))?);
                } else if let Some(n) = a.strip_prefix("-threads=") {
                    o.threads = n.parse().map_err(|_| format!("bad -threads value: {}", n))?;
                } else if a.starts_with("-m680") || a.starts_with("-m68") || a.starts_with("-mcf") || a.starts_with("-m") {
                    return Err(format!("only -m68000 is supported: {}", a));
                } else if a.starts_with("-opt-") || a.starts_with("-sdreg=") {
                    return Err(format!("option not supported by vasm_m: {}", a));
                } else {
                    return Err(format!("unknown option: {}", a));
                }
            }
        }
        i += 1;
    }
    Ok(o)
}

fn parse_int(s: &str) -> Result<i64, String> {
    let t = s.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let v = if let Some(h) = t.strip_prefix('$') {
        i64::from_str_radix(h, 16)
    } else if let Some(h) = t.strip_prefix("0x") {
        i64::from_str_radix(h, 16)
    } else if let Some(b) = t.strip_prefix('%') {
        i64::from_str_radix(b, 2)
    } else {
        t.parse::<i64>()
    }
    .map_err(|_| format!("bad integer: {}", s))?;
    Ok(if neg { -v } else { v })
}
