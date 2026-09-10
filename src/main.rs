mod asm;
mod atoms;
mod chars;
mod cli;
mod errors;
mod errtab;
mod expr;
mod m68k;
mod output;
mod resolve;
mod source;
mod symbols;
mod syntax;
mod types;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match cli::parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {}", e);
            eprint!("{}", cli::usage());
            return ExitCode::from(1);
        }
    };
    let quiet = opts.quiet;
    if !quiet {
        eprintln!("vasm_m 0.1.0 (68000, compatible with vasm 1.7h / vasmm68k_mot)");
    }
    let mut a = asm::Assembler::new(opts);
    let Some(input) = a.opts.input.clone() else {
        a.general_error(15, &[]);
        return ExitCode::from(1);
    };
    // main(): internal_abs("__VASM"), init_parse, init_syntax, init_cpu, set_input_name
    a.internal_abs("__VASM");
    a.init_syntax();
    let ct = a.cpu.cpu_type;
    a.set_internal_abs("__VASM", (ct & m68k::tables::CPUMASK) as i32);
    for (name, val) in a.opts.defines.clone() {
        a.new_abs(&name, expr::Expr::Num(val as i32));
    }
    let timing = std::env::var("VASM_M_TIMING").is_ok();
    let t0 = std::time::Instant::now();
    a.set_input_name(&input);
    a.parse();
    let t1 = std::time::Instant::now();
    if a.errs.errors == 0 {
        a.resolve();
    }
    let t2 = std::time::Instant::now();
    if a.errs.errors == 0 {
        a.assemble();
    }
    let t3 = std::time::Instant::now();
    if timing {
        eprintln!("timing: parse {:?} resolve {:?} assemble {:?}; memo hits {} misses {}", t1 - t0, t2 - t1, t3 - t2, a.memo_hits, a.memo_misses);
    }
    if a.errs.errors == 0 {
        a.undef_syms();
    }
    a.fix_labels();
    if std::env::var("VASM_M_SYMS").is_ok() {
        for s in &a.symtab.syms {
            eprintln!("sym {:?} kind={:?} sec={:?} pc={:#x} ver={}", s.name, s.kind, s.sec, s.pc, s.version);
        }
    }
    if a.errs.errors == 0 {
        if !quiet {
            for s in &a.sections {
                if s.flags & atoms::UNALLOCATED == 0 {
                    eprintln!("{}({}{}):\t{:12} bytes", s.name, s.attr, s.align, (s.pc as u32).wrapping_sub(s.org as u32));
                }
            }
        }
        match a.opts.format {
            cli::OutputFormat::Bin => {
                let t4 = std::time::Instant::now();
                let data = a.write_bin();
                if a.errs.errors == 0 {
                    a.write_output_file(&data);
                }
                if timing {
                    eprintln!("timing: output {:?}", t4.elapsed());
                }
            }
            _ => {
                a.general_error(16, &[errors::Arg::from("hunk/test not implemented")]);
            }
        }
    }
    a.leave()
}
