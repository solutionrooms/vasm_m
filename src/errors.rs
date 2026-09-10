//! Error reporting, transliterated from vasm 1.7h error.c.
use crate::asm::Assembler;
use crate::errtab::*;

pub enum Arg {
    S(String),
    D(i64),
    C(u8),
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self { Arg::S(s.to_string()) }
}
impl From<String> for Arg {
    fn from(s: String) -> Self { Arg::S(s) }
}
impl From<i32> for Arg {
    fn from(v: i32) -> Self { Arg::D(v as i64) }
}
impl From<i64> for Arg {
    fn from(v: i64) -> Self { Arg::D(v) }
}
impl From<usize> for Arg {
    fn from(v: usize) -> Self { Arg::D(v as i64) }
}
impl From<u32> for Arg {
    fn from(v: u32) -> Self { Arg::D(v as i64) }
}
impl From<u8> for Arg {
    fn from(v: u8) -> Self { Arg::C(v) }
}

fn format_msg(fmt: &str, args: &[Arg]) -> String {
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    let mut ai = 0;
    while let Some(c) = it.next() {
        if c == '%' {
            // skip length modifiers (l, ll, h, z) before the conversion char
            while matches!(it.peek(), Some('l' | 'h' | 'z')) {
                it.next();
            }
            match it.next() {
                Some('%') => out.push('%'),
                Some(_spec) => {
                    match args.get(ai) {
                        Some(Arg::S(s)) => out.push_str(s),
                        Some(Arg::D(d)) => out.push_str(&d.to_string()),
                        Some(Arg::C(ch)) => out.push(*ch as char),
                        None => out.push('?'),
                    }
                    ai += 1;
                }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Default)]
pub struct ErrorState {
    pub errors: u32,
    pub warnings: u32,
    pub max_errors: u32,
    pub no_warn: bool,
    pub disabled: Vec<u32>,
    pub range_warnings: bool,
    last_src: Option<usize>,
    last_line: i32,
    last_no: u32,
}

impl Assembler {
    pub fn general_error(&mut self, n: u32, args: &[Arg]) {
        self.report(GENERAL_ERRORS, n, FIRST_GENERAL_ERROR, args);
    }
    pub fn syntax_error(&mut self, n: u32, args: &[Arg]) {
        self.report(SYNTAX_ERRORS, n, FIRST_SYNTAX_ERROR, args);
    }
    pub fn cpu_error(&mut self, n: u32, args: &[Arg]) {
        self.report(CPU_ERRORS, n, FIRST_CPU_ERROR, args);
    }
    pub fn output_error(&mut self, n: u32, args: &[Arg]) {
        self.report(OUTPUT_ERRORS, n, FIRST_OUTPUT_ERROR, args);
    }
    pub fn ierror(&mut self, code: i32, file: &str, line: u32) -> ! {
        self.general_error(4, &[Arg::D(code as i64), Arg::D(line as i64), Arg::S(file.to_string())]);
        // general error 4 is FATAL; report() exits. Unreachable in practice.
        std::process::exit(1)
    }

    fn report(&mut self, table: &[(&str, u32)], n: u32, offset: u32, args: &[Arg]) {
        let (text, mut flags) = table[n as usize];
        if self.errs.range_warnings && offset == FIRST_CPU_ERROR && matches!(n, 25 | 29 | 32 | 36) {
            flags = (flags & !ERROR) | WARNING;
        }
        let st = &mut self.errs;
        if flags & DONTWARN != 0 || (flags & WARNING != 0 && st.no_warn) {
            return;
        }
        if flags & WARNING != 0 && st.disabled.contains(&(n + offset)) {
            return;
        }
        let is_msg = flags & MESSAGE != 0 && flags & (WARNING | ERROR | FATAL) == 0;
        let cur = self.cur_src;
        let (cur_line, cur_name) = match cur {
            Some(i) => (self.sources[i].line, self.sources[i].name.clone()),
            None => (0, String::new()),
        };
        if !is_msg {
            if let Some(ls) = st.last_src {
                if let Some(c) = cur {
                    if c == ls && cur_line == st.last_line && n + offset == st.last_no {
                        return;
                    }
                }
            }
        }
        if cur.is_some() {
            st.last_src = cur;
            st.last_line = cur_line;
            st.last_no = n + offset;
        }
        let mut out = String::from("\n");
        if flags & FATAL != 0 {
            out.push_str("fatal ");
        }
        if flags & ERROR != 0 {
            st.errors += 1;
            out.push_str("error");
        } else if flags & WARNING != 0 {
            st.warnings += 1;
            out.push_str("warning");
        } else if flags & MESSAGE != 0 {
            out.push_str("message");
        }
        out.push_str(&format!(" {}", n + offset));
        if flags & NOLINE == 0 && cur.is_some() {
            out.push_str(&format!(" in line {} of \"{}\"", cur_line, cur_name));
        }
        out.push_str(": ");
        out.push_str(&format_msg(text, args));
        out.push('\n');
        if flags & NOLINE == 0 {
            if let Some(mut child) = cur {
                while let Some(parent) = self.sources[child].parent {
                    let cs = &self.sources[child];
                    out.push_str(if cs.num_params >= 0 { "\tcalled" } else { "\tincluded" });
                    out.push_str(&format!(" from line {} of \"{}\"", cs.parent_line, self.sources[parent].name));
                    let mut recurs = 1;
                    let mut p = parent;
                    while let Some(pp) = self.sources[p].parent {
                        if self.sources[child].parent_line == self.sources[p].parent_line
                            && self.sources[p].name == self.sources[pp].name
                        {
                            recurs += 1;
                            p = pp;
                        } else {
                            break;
                        }
                    }
                    if recurs > 1 {
                        out.push_str(&format!(" {} times", recurs));
                    }
                    out.push('\n');
                    child = p;
                }
                // print_source_line: the current line buffer
                let l = &self.line;
                if l.len() > 1 {
                    let end = l.iter().position(|&b| b == 0).unwrap_or(l.len());
                    // vasm prints the original source line; our buffer may be
                    // truncated by exp_skip, which is close enough for diagnostics.
                    out.push('>');
                    out.push_str(&crate::chars::bytes_to_string(&l[1..end.max(1)]));
                    out.push('\n');
                }
            }
        }
        if is_msg {
            print!("{}", out);
        } else {
            eprint!("{}", out);
        }
        if flags & FATAL != 0 {
            eprintln!("aborting...");
            self.leave();
        }
        if flags & ERROR != 0 && self.errs.max_errors != 0 && self.errs.errors >= self.errs.max_errors {
            eprintln!("***maximum number of errors reached!***");
            self.leave();
        }
    }

    /// vasm.c leave(): close/remove output on error, exit.
    pub fn leave(&mut self) -> ! {
        if self.errs.errors != 0 {
            if let Some(o) = &self.opts.output {
                let _ = std::fs::remove_file(o);
            }
        }
        std::process::exit(if self.errs.errors != 0 { 1 } else { 0 })
    }
}
