//! Source text instances, line reading, macro/repeat bodies and expansion.
//! Transliterated from vasm 1.7h vasm.c (new_source, include_source, ...),
//! parse.c (read_next_line, execute_macro, new_macro, new_repeat, ...) and
//! syntax/mot/syntax.c (expand_macro, parse_macro_arg, my_exec_macro).
use crate::asm::{Assembler, IDSTACKSIZE};
use crate::atoms::{Atom, DBlock};
use crate::chars::*;
use crate::errors::Arg;
use crate::expr::{Expr, Op};
use crate::symbols::SymKind;
use std::rc::Rc;

pub const INITLINELEN: usize = 256;
pub const SRCREADINC: usize = 65536;
pub const CARGSYM: &str = "CARG";
pub const REPTNSYM: &str = "REPTN";

pub static ENDM_DIRLIST: &[&[u8]] = &[b"endm"];
pub static REPT_DIRLIST: &[&[u8]] = &[b"rept"];
pub static ENDR_DIRLIST: &[&[u8]] = &[b"endr"];
pub static EREM_DIRLIST: &[&[u8]] = &[b"erem"];

pub struct Source {
    pub parent: Option<usize>,
    pub parent_line: i32,
    pub name: String,
    pub text: Rc<Vec<u8>>,
    pub start: usize,
    pub end: usize,
    pub macro_id: Option<usize>,
    pub repeat: i64,
    pub cond_level: usize,
    pub num_params: i32,
    pub params: Vec<Vec<u8>>,
    pub quals: Vec<Vec<u8>>,
    pub num_quals: usize,
    pub id: u64,
    pub srcptr: usize,
    pub line: i32,
    pub reptn: i64,
    pub cargexp: Option<Rc<Expr>>,
}

pub struct Macro {
    pub name: String,
    pub text: Rc<Vec<u8>>,
    pub start: usize,
    pub size: usize,
    pub valid: bool,
    pub recursions: u32,
    pub vararg: i32,
    pub num_argnames: i32,
}

impl Assembler {
    // ----- files -------------------------------------------------------------
    /// convert_path(): backslashes to slashes (Unix host)
    pub fn convert_path(p: &str) -> String {
        p.replace('\\', "/")
    }

    /// new_include_path()
    pub fn new_include_path(&mut self, pathname: &str) {
        let mut p = Self::convert_path(pathname);
        if !p.is_empty() && !p.ends_with('/') {
            p.push('/');
        }
        self.include_paths.push(p);
    }

    /// locate_file(): returns the path that opened, or reports error 12 (fatal)
    pub fn locate_file(&mut self, filename: &str) -> Option<Vec<u8>> {
        let f0 = filename.as_bytes().first().copied().unwrap_or(0);
        if f0 == b'.' || f0 == b'/' || f0 == b'\\' || filename.contains(':') {
            if let Ok(d) = std::fs::read(filename) {
                return Some(d);
            }
        } else {
            for ip in self.include_paths.clone() {
                let full = format!("{}{}", ip, filename);
                if let Ok(d) = std::fs::read(&full) {
                    return Some(d);
                }
            }
        }
        self.general_error(12, &[Arg::from(filename)]);
        None
    }

    /// set_input_name()
    pub fn set_input_name(&mut self, inname: &str) {
        if let Some(p) = inname.rfind(['/', '\\', ':']) {
            let dir = inname[..=p].to_string();
            self.new_include_path(&dir);
        }
        self.filename = Some(inname.to_string());
        self.include_source(inname);
    }

    /// include_source()
    pub fn include_source(&mut self, inname: &str) {
        let filename = Self::convert_path(inname);
        let already = self.included.iter().any(|p| *p == filename);
        if !already {
            self.included.push(filename.clone());
        } else if self.ignore_multinc {
            return;
        }
        if let Some(mut text) = self.locate_file(&filename) {
            if text.is_empty() {
                text = b"\n".to_vec();
            } else {
                text.push(b'\n');
            }
            let size = text.len();
            let s = self.new_source(&filename, Rc::new(text), 0, size);
            self.cur_src = Some(s);
        }
    }

    /// include_binary_file(name,0,0)
    pub fn include_binary_file(&mut self, inname: &str) {
        let filename = Self::convert_path(inname);
        if let Some(data) = self.locate_file(&filename) {
            if !data.is_empty() {
                self.add_atom(Atom::data(DBlock::new(data), 1));
            }
        }
    }

    /// new_source(): text[start..end) becomes a new source with cur_src as parent
    pub fn new_source(&mut self, name: &str, text: Rc<Vec<u8>>, start: usize, mut end: usize) -> usize {
        // Ctrl-Z: EOF character - treat as newline and ignore the rest
        if let Some(i) = text[start..end].iter().position(|&b| b == 0x1a) {
            // vasm overwrites the byte with '\n'; we keep text immutable and
            // instead end the source right before it with an implied newline.
            let mut t = text[start..start + i].to_vec();
            t.push(b'\n');
            let n = t.len();
            let s = Source {
                parent: self.cur_src,
                parent_line: self.cur_src.map(|c| self.sources[c].line).unwrap_or(0),
                name: name.to_string(),
                text: Rc::new(t),
                start: 0,
                end: n,
                macro_id: None,
                repeat: 1,
                cond_level: self.cond.clev,
                num_params: -1,
                params: vec![Vec::new(); self.maxmacparams],
                quals: vec![Vec::new(); 1],
                num_quals: 0,
                id: self.source_id,
                srcptr: 0,
                line: 0,
                reptn: self.cur_src.map(|c| self.sources[c].reptn).unwrap_or(-1),
                cargexp: None,
            };
            self.source_id += 1;
            self.sources.push(s);
            return self.sources.len() - 1;
        }
        if end > text.len() {
            end = text.len();
        }
        let s = Source {
            parent: self.cur_src,
            parent_line: self.cur_src.map(|c| self.sources[c].line).unwrap_or(0),
            name: name.to_string(),
            text,
            start,
            end,
            macro_id: None,
            repeat: 1,
            cond_level: self.cond.clev,
            num_params: -1,
            params: vec![Vec::new(); self.maxmacparams],
            quals: vec![Vec::new(); 1],
            num_quals: 0,
            id: self.source_id,
            srcptr: start,
            line: 0,
            reptn: self.cur_src.map(|c| self.sources[c].reptn).unwrap_or(-1),
            cargexp: None,
        };
        self.source_id += 1;
        self.sources.push(s);
        self.sources.len() - 1
    }

    /// end_source()
    pub fn end_source(&mut self, s: usize) {
        let src = &mut self.sources[s];
        src.srcptr = src.end;
        src.repeat = 1;
        self.cond.clev = src.cond_level;
    }

    /// real_line()
    pub fn real_line(&self) -> i32 {
        let mut src = self.cur_src.unwrap();
        let mut line = self.sources[src].line;
        while self.sources[src].num_params >= 0 {
            line = self.sources[src].parent_line;
            src = self.sources[src].parent.expect("macro must have a parent");
        }
        line
    }

    // ----- macros / repeats ---------------------------------------------------
    /// find_macro()
    pub fn find_macro(&self, name: &[u8]) -> Option<usize> {
        let key = if self.nocase_macros { lower_string(name) } else { bytes_to_string(name) };
        self.macro_map.get(&key).copied()
    }

    /// new_macro(): start a macro definition (body is captured by read_next_line)
    pub fn new_macro(&mut self, name: &str) {
        if self.cur_macro.is_none() && self.cur_src.is_some() && self.enddir_list.is_none() {
            let cs = self.cur_src.unwrap();
            let mut valid = true;
            let lname = name.to_ascii_lowercase();
            if crate::m68k::parse::mnemonic_exists(&lname) {
                self.general_error(51, &[]);
                valid = false;
            } else if crate::syntax::directive_exists(&lname) {
                self.general_error(52, &[]);
                valid = false;
            }
            let m = Macro {
                name: if self.nocase_macros { lname } else { name.to_string() },
                text: self.sources[cs].text.clone(),
                start: self.sources[cs].srcptr,
                size: 0,
                valid,
                recursions: 0,
                vararg: -1,
                num_argnames: -1,
            };
            self.macros.push(m);
            self.cur_macro = Some(self.macros.len() - 1);
            self.enddir_list = Some(ENDM_DIRLIST);
            self.rept_cnt = -1;
            self.rept_start = 0;
        } else {
            self.ierror(0, "source.rs", line!());
        }
    }

    /// add_macro(): link the finished definition into the table
    fn add_macro(&mut self) {
        if let (Some(m), Some(cs)) = (self.cur_macro, self.cur_src) {
            if self.macros[m].valid {
                self.macros[m].size = self.sources[cs].srcptr - self.macros[m].start;
                let key = self.macros[m].name.clone();
                self.macro_map.insert(key, m);
            }
            self.cur_macro = None;
        } else {
            self.ierror(0, "source.rs", line!());
        }
    }

    /// new_repeat()
    pub fn new_repeat(&mut self, rcnt: i32, reptlist: Option<&'static [&'static [u8]]>, endrlist: &'static [&'static [u8]]) {
        if self.cur_macro.is_none() && self.cur_src.is_some() && self.enddir_list.is_none() {
            self.enddir_list = Some(endrlist);
            self.reptdir_list = reptlist;
            self.rept_cnt = rcnt;
            self.rept_start = self.sources[self.cur_src.unwrap()].srcptr;
        } else {
            self.ierror(0, "source.rs", line!());
        }
    }

    /// start_repeat()
    fn start_repeat(&mut self, rept_end: usize) {
        self.reptdir_list = None;
        let cs = self.cur_src.unwrap();
        if self.rept_cnt < 0 {
            self.ierror(0, "source.rs", line!());
        }
        if self.rept_cnt > 0 {
            let name = format!("REPEAT:{}:line {}", self.sources[cs].name, self.sources[cs].line);
            let text = self.sources[cs].text.clone();
            let s = self.new_source(&name, text, self.rept_start, rept_end);
            self.sources[s].repeat = self.rept_cnt as i64;
            self.sources[s].reptn = 0;
            self.set_internal_abs(REPTNSYM, 0);
            if self.sources[cs].num_params >= 0 {
                let np = self.sources[cs].num_params;
                let params = self.sources[cs].params.clone();
                let quals = self.sources[cs].quals.clone();
                let nq = self.sources[cs].num_quals;
                let d = &mut self.sources[s];
                d.num_params = np;
                d.params = params;
                d.quals = quals;
                d.num_quals = nq;
            }
            self.cur_src = Some(s);
        }
    }

    /// leave_macro()
    pub fn leave_macro(&mut self) -> bool {
        let cs = self.cur_src.unwrap();
        if self.sources[cs].macro_id.is_some() {
            self.end_source(cs);
            true
        } else {
            self.general_error(36, &[]);
            false
        }
    }

    /// parse_macro_arg(): returns (param bytes, new pos)
    fn parse_macro_arg(&mut self, mut s: usize) -> (Vec<u8>, usize) {
        let start = s;
        if self.line[s] == b'<' {
            let pstart = s + 1;
            loop {
                s += 1;
                let c = self.line[s];
                if c == 0 {
                    break;
                }
                if c == b'>' {
                    if self.line[s + 1] == b'>' {
                        // convert ">>" into a single ">" by shifting the line buffer
                        self.line.remove(s + 1);
                    } else {
                        let p = self.line[pstart..s].to_vec();
                        return (p, s + 1);
                    }
                }
            }
            // unterminated: vasm leaves param.len = 0 (uninitialised in C)
            (Vec::new(), s)
        } else if self.line[s] == b'"' || self.line[s] == b'\'' {
            let e = self.skip_string(s, self.line[s]);
            (self.line[start..e].to_vec(), e)
        } else {
            let e = self.skip_operand(s);
            let t = trim_end(&self.line, start, e);
            (self.line[start..t].to_vec(), e)
        }
    }

    /// execute_macro(): returns true when `name` was a macro and it was entered
    pub fn execute_macro(&mut self, name_start: usize, name_len: usize, quals: &[(usize, usize)], s: usize) -> bool {
        let name = self.line[name_start..name_start + name_len].to_vec();
        let m = match self.find_macro(&name) {
            Some(m) => m,
            None => return false,
        };
        if self.macros[m].recursions >= self.maxmacrecurs {
            self.general_error(56, &[Arg::from(self.maxmacrecurs)]);
            return false;
        }
        self.macros[m].recursions += 1;
        let (text, start, size, mname) = {
            let mm = &self.macros[m];
            (mm.text.clone(), mm.start, mm.size, mm.name.clone())
        };
        let src = self.new_source(&mname, text, start, start + size);
        // qualifiers: given ones, then cpu default ("w")
        let mut nq = 0;
        for (qs, ql) in quals {
            self.sources[src].quals[nq] = self.line[*qs..*qs + *ql].to_vec();
            nq += 1;
        }
        if nq == 0 {
            self.sources[src].quals[0] = b"w".to_vec();
            nq = 1;
        }
        self.sources[src].num_quals = nq;
        // read arguments
        let mut s = skip(&self.line, s);
        let mut n: i32 = 0;
        while !is_eol(&self.line, s) && (n as usize) < self.maxmacparams {
            let (param, ns) = self.parse_macro_arg(s);
            s = ns;
            if n >= 0 {
                if !param.is_empty() {
                    self.sources[src].params[n as usize] = param;
                }
                n += 1;
            }
            s = skip(&self.line, s);
            // MACRO_PARAM_SEP
            if self.line[s] == b',' {
                s = skip(&self.line, s + 1);
            } else {
                break;
            }
        }
        if n as usize > self.maxmacparams {
            self.general_error(27, &[Arg::from(self.maxmacparams)]);
            n = self.maxmacparams as i32;
        }
        self.sources[src].macro_id = Some(m);
        self.sources[src].num_params = n;
        // EXEC_MACRO: reset CARG to 1, remember parent's CARG expression
        let carg = self.internal_abs(CARGSYM);
        let cs = self.cur_src.unwrap();
        self.sources[cs].cargexp = self.symtab.syms[carg].expr.clone();
        self.symtab.syms[carg].expr = Some(Rc::new(Expr::Num(1)));
        self.cur_src = Some(src);
        true
    }

    /// copy_macro_param(): returns bytes for parameter n (0-based) or None for "no such"
    fn macro_param(&self, src: usize, n: i32) -> Vec<u8> {
        if n < 0 {
            return Vec::new();
        }
        let s = &self.sources[src];
        if n < s.num_params && (n as usize) < self.maxmacparams {
            s.params[n as usize].clone()
        } else {
            Vec::new()
        }
    }

    fn macro_qual(&self, src: usize, n: usize) -> Vec<u8> {
        let s = &self.sources[src];
        if n < s.num_quals { s.quals[n].clone() } else { Vec::new() }
    }

    /// copy_macro_carg()
    fn macro_carg(&mut self, src: usize, inc: i32) -> Vec<u8> {
        let carg = self.internal_abs(CARGSYM);
        if self.symtab.syms[carg].kind != SymKind::Expression {
            return Vec::new();
        }
        let mut e = self.symtab.syms[carg].expr.as_ref().map(|e| (**e).clone()).unwrap_or(Expr::Num(0));
        self.simplify_expr(&mut e);
        let val = match e {
            Expr::Num(v) => v,
            _ => {
                self.general_error(30, &[]);
                return Vec::new();
            }
        };
        self.symtab.syms[carg].expr = Some(Rc::new(Expr::Num(val)));
        let r = self.macro_param(src, val - 1);
        if inc != 0 {
            let op = if inc > 0 { Op::Add } else { Op::Sub };
            let mut ne = Expr::Bin(op, Box::new(Expr::Num(val)), Box::new(Expr::Num(1)));
            self.simplify_expr(&mut ne);
            self.symtab.syms[carg].expr = Some(Rc::new(ne));
        }
        r
    }

    /// expand_macro(): try to expand a `\` sequence at text[*s]. Returns
    /// Some(bytes) when an expansion took place (and advances *s), None otherwise.
    fn expand_macro(&mut self, src: usize, text: &[u8], s: &mut usize) -> Option<Vec<u8>> {
        if text[*s] != b'\\' {
            return None;
        }
        let mut p = *s + 1;
        let c = text.get(p).copied().unwrap_or(0);
        let out: Vec<u8>;
        if c == b'\\' {
            p += 1;
            out = if self.esc_sequences { b"\\\\".to_vec() } else { b"\\".to_vec() };
        } else if c == b'@' {
            p += 1;
            let unique_id;
            if text.get(p).copied() == Some(b'@') {
                if self.id_stack.is_empty() {
                    self.syntax_error(17, &[]);
                    return Some(Vec::new());
                }
                unique_id = *self.id_stack.last().unwrap();
            } else {
                unique_id = self.sources[src].id;
            }
            out = format!("_{:06}", unique_id).into_bytes();
            match text.get(p).copied() {
                Some(b'!') => {
                    if self.id_stack.len() >= IDSTACKSIZE {
                        self.syntax_error(16, &[]);
                        return Some(Vec::new());
                    }
                    self.id_stack.push(unique_id);
                    p += 1;
                }
                Some(b'?') => {
                    if self.id_stack.len() >= IDSTACKSIZE {
                        self.syntax_error(16, &[]);
                        return Some(Vec::new());
                    }
                    if self.id_stack.is_empty() {
                        self.syntax_error(14, &[]);
                        return Some(Vec::new());
                    }
                    let top = *self.id_stack.last().unwrap();
                    let n = self.id_stack.len();
                    self.id_stack[n - 1] = unique_id;
                    self.id_stack.push(top);
                    p += 1;
                }
                Some(b'@') => {
                    self.id_stack.pop();
                    p += 1;
                }
                _ => {}
            }
        } else if c == b'<' {
            p += 1;
            let hex = text.get(p).copied() == Some(b'$');
            if hex {
                p += 1;
            }
            // parse_symbol on the raw text: temporarily use a scratch line
            let saved = std::mem::take(&mut self.line);
            let mut tmp = vec![0u8];
            tmp.extend_from_slice(&text[p..]);
            tmp.push(0);
            tmp.push(0);
            tmp.push(0);
            self.line = tmp;
            let r = self.parse_symbol(1);
            let mut val: Option<u32> = None;
            let mut np = p;
            if let Some((name, e)) = r {
                np = p + (e - 1);
                if let Some(sym) = self.find_symbol(&name) {
                    if self.symtab.syms[sym].kind == SymKind::Expression {
                        let ex = self.symtab.syms[sym].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                        let (v, cnst) = self.eval_expr(&ex, None, 0);
                        if cnst {
                            val = Some(v as u32);
                        }
                    }
                }
                self.line = saved;
                let close = text.get(np).copied();
                np += 1;
                if close != Some(b'>') || val.is_none() {
                    self.syntax_error(19, &[]);
                    return Some(Vec::new());
                }
                let v = val.unwrap();
                out = if hex { format!("{:X}", v).into_bytes() } else { v.to_string().into_bytes() };
                p = np;
            } else {
                self.line = saved;
                self.syntax_error(10, &[]);
                return Some(Vec::new());
            }
        } else if c == b'#' {
            p += 1;
            out = self.sources[src].num_params.to_string().into_bytes();
        } else if c == b'?' && text.get(p + 1).map(|b| b.is_ascii_digit()).unwrap_or(false) {
            let d = text[p + 1];
            let n = if d == b'0' {
                self.sources[src].quals[0].len()
            } else {
                let i = (d - b'1') as usize;
                let s = &self.sources[src];
                if i < s.params.len() { s.params[i].len() } else { 0 }
            };
            out = n.to_string().into_bytes();
            p += 2;
        } else if c == b'.' {
            p += 1;
            out = self.macro_carg(src, 0);
        } else if c == b'+' {
            p += 1;
            out = self.macro_carg(src, 1);
        } else if c == b'-' {
            p += 1;
            out = self.macro_carg(src, -1);
        } else if c.is_ascii_digit() {
            out = if c == b'0' { self.macro_qual(src, 0) } else { self.macro_param(src, (c - b'1') as i32) };
            p += 1;
        } else if self.maxmacparams > 9 && c.to_ascii_lowercase() >= b'a' && (c.to_ascii_lowercase() as usize) < (b'a' as usize + self.maxmacparams - 9) {
            out = self.macro_param(src, (c.to_ascii_lowercase() - b'a') as i32 + 9);
            p += 1;
        } else {
            return None;
        }
        *s = p;
        Some(out)
    }

    /// dirlist_match(): does one of the directives match at text[s]?
    fn dirlist_match(text: &[u8], s: usize, end: usize, list: &[&[u8]]) -> Option<usize> {
        let maxlen = end - s;
        for d in list {
            if d.len() <= maxlen && eq_nocase_slice(&text[s..s + d.len()], d) && s + d.len() < text.len() && is_space(text[s + d.len()]) {
                return Some(d.len());
            }
        }
        None
    }

    fn skip_eol(text: &[u8], mut s: usize, e: usize) -> usize {
        while s < e && text[s] != 0 && text[s] != b'\n' && text[s] != b'\r' {
            s += 1;
        }
        s
    }

    /// read_next_line(): fills self.line ([0, text, 0]); returns false at end of assembly
    pub fn read_next_line(&mut self) -> bool {
        // end of source reached?
        loop {
            let cs = self.cur_src.unwrap();
            let src = &self.sources[cs];
            if src.srcptr >= src.end || src.text[src.srcptr] == 0 {
                let src = &mut self.sources[cs];
                src.repeat -= 1;
                if src.repeat > 0 {
                    src.srcptr = src.start;
                    src.line = 0;
                    src.reptn += 1;
                    let r = src.reptn;
                    self.set_internal_abs(REPTNSYM, r as i32);
                } else {
                    if let Some(m) = src.macro_id {
                        if self.macros[m].recursions == 0 {
                            self.ierror(0, "source.rs", line!());
                        }
                        self.macros[m].recursions -= 1;
                    }
                    let parent = self.sources[cs].parent;
                    match parent {
                        None => return false,
                        Some(p) => {
                            self.cur_src = Some(p);
                            if let Some(ce) = self.sources[p].cargexp.take() {
                                let carg = self.internal_abs(CARGSYM);
                                self.symtab.syms[carg].expr = Some(ce);
                            }
                            let r = self.sources[p].reptn;
                            self.set_internal_abs(REPTNSYM, r as i32);
                        }
                    }
                }
            } else {
                break;
            }
        }
        let cs = self.cur_src.unwrap();
        self.sources[cs].line += 1;
        let text = self.sources[cs].text.clone();
        let srcend = self.sources[cs].end;
        let mut s = self.sources[cs].srcptr;
        let nparam = self.sources[cs].num_params;
        let mut rept_end: Option<usize> = None;
        let mut d: Vec<u8> = std::mem::take(&mut self.line);
        d.clear();
        d.push(0);

        if let Some(enddir) = self.enddir_list {
            let enddir_minlen = enddir.iter().map(|x| x.len()).min().unwrap_or(0);
            if srcend - s > enddir_minlen {
                let mut rept_nest = 1;
                if nparam >= 0 && self.cur_macro.is_some() {
                    let n = self.sources[cs].name.clone();
                    self.general_error(26, &[Arg::from(n)]);
                }
                while s + enddir_minlen <= srcend {
                    if let Some(len) = Self::dirlist_match(&text, s, srcend, enddir) {
                        if self.cur_macro.is_some() {
                            // link macro definition: size up to here
                            self.sources[cs].srcptr = s;
                            self.add_macro();
                            s += len;
                            self.enddir_list = None;
                            break;
                        } else {
                            rept_nest -= 1;
                            if rept_nest == 0 {
                                rept_end = Some(s);
                                s += len;
                                self.enddir_list = None;
                                break;
                            }
                        }
                    } else if self.cur_macro.is_none() {
                        if let Some(rl) = self.reptdir_list {
                            if let Some(len) = Self::dirlist_match(&text, s, srcend, rl) {
                                s += len;
                                rept_nest += 1;
                            }
                        }
                    }
                    if text[s] == b'"' || text[s] == b'\'' {
                        let c = text[s];
                        s += 1;
                        while s + enddir_minlen <= srcend && text[s] != c && text[s] != b'\n' && text[s] != b'\r' {
                            if text[s] == b'\\' && (text[s + 1] == b'"' || text[s + 1] == b'\'') {
                                s = self.escape_text(&text, s).0;
                            } else {
                                s += 1;
                            }
                        }
                    }
                    if text[s] == 0 || text[s] == b';' {
                        s = Self::skip_eol(&text, s, srcend);
                    }
                    if text[s] == b'\n' {
                        self.sources[cs].srcptr = s + 1;
                        self.sources[cs].line += 1;
                    } else if text[s] == b'\r' && (s == 0 || text[s - 1] != b'\n') && (s >= srcend - 1 || text[s + 1] != b'\n') {
                        self.sources[cs].srcptr = s + 1;
                        self.sources[cs].line += 1;
                    }
                    s += 1;
                }
                if self.enddir_list.is_some() {
                    if let Some(m) = self.cur_macro {
                        let n = self.macros[m].name.clone();
                        self.general_error(25, &[Arg::from(n)]);
                    } else {
                        self.general_error(32, &[]);
                    }
                }
                // ignore rest of line, treat as comment
                s = Self::skip_eol(&text, s, srcend);
            }
        }

        // copy next line to the line buffer, expanding macro arguments
        let srcptr0 = self.sources[cs].srcptr;
        while s < srcend && text[s] != 0 {
            if nparam >= 0 {
                if let Some(exp) = self.expand_macro(cs, &text, &mut s) {
                    d.extend_from_slice(&exp);
                    continue;
                }
            }
            let c = text[s];
            if c == b'\r' {
                if (s > srcptr0 && text[s - 1] == b'\n') || (s < srcend - 1 && text[s + 1] == b'\n') {
                    s += 1;
                } else {
                    s += 1;
                    break;
                }
            } else if c == b'\n' {
                s += 1;
                break;
            } else {
                d.push(c);
                s += 1;
            }
        }
        d.push(0);
        d.push(0);
        d.push(0);
        self.line = d;
        self.sources[cs].srcptr = s;
        if let Some(re) = rept_end {
            self.start_repeat(re);
        }
        true
    }

    // ----- string helpers (parse.c) --------------------------------------------
    /// escape() on the line buffer: returns (new pos, code)
    pub fn escape(&mut self, s: usize) -> (usize, u8) {
        let text = std::mem::take(&mut self.line);
        let r = self.escape_text(&text, s);
        self.line = text;
        r
    }

    /// escape() on arbitrary text
    pub fn escape_text(&mut self, text: &[u8], s: usize) -> (usize, u8) {
        let mut s = s + 1; // skip backslash
        if !self.esc_sequences {
            return (s, b'\\');
        }
        let c = text.get(s).copied().unwrap_or(0);
        match c {
            b'b' => (s + 1, 8),
            b'f' => (s + 1, 12),
            b'n' => (s + 1, b'\n'),
            b'r' => (s + 1, b'\r'),
            b't' => (s + 1, b'\t'),
            b'\\' => (s + 1, b'\\'),
            b'"' => (s + 1, b'"'),
            b'\'' => (s + 1, b'\''),
            b'e' => (s + 1, 27),
            b'0'..=b'9' => {
                let mut code: u8 = 0;
                let mut cnt = 0;
                while text.get(s).map(|b| b.is_ascii_digit()).unwrap_or(false) && { cnt += 1; cnt <= 3 } {
                    code = code.wrapping_mul(8).wrapping_add(text[s] - b'0');
                    s += 1;
                }
                (s, code)
            }
            b'x' | b'X' => {
                let mut code: u8 = 0;
                s += 1;
                while let Some(&b) = text.get(s) {
                    let v = match b {
                        b'0'..=b'9' => b - b'0',
                        b'a'..=b'f' => b - b'a' + 10,
                        b'A'..=b'F' => b - b'A' + 10,
                        _ => break,
                    };
                    code = code.wrapping_mul(16).wrapping_add(v);
                    s += 1;
                }
                (s, code)
            }
            _ => {
                self.general_error(35, &[Arg::C(c)]);
                (s, c)
            }
        }
    }

    /// skip_string(): returns index after the closing delimiter
    pub fn skip_string(&mut self, mut s: usize, delim: u8) -> usize {
        if self.line[s] != delim {
            self.general_error(6, &[Arg::C(delim)]);
        } else {
            s += 1;
        }
        while self.line[s] != 0 {
            if self.line[s] == b'\\' {
                s = self.escape(s).0;
            } else {
                let c = self.line[s];
                s += 1;
                if c == delim {
                    if self.line[s] == delim {
                        s += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        if s == 0 || self.line[s - 1] != delim {
            self.general_error(6, &[Arg::C(delim)]);
        }
        s
    }

    /// skip_string with size counting (number of characters)
    fn string_size(&mut self, mut s: usize, delim: u8) -> usize {
        let mut n = 0;
        if self.line[s] == delim {
            s += 1;
        }
        while self.line[s] != 0 {
            if self.line[s] == b'\\' {
                s = self.escape(s).0;
            } else {
                let c = self.line[s];
                s += 1;
                if c == delim {
                    if self.line[s] == delim {
                        s += 1;
                    } else {
                        break;
                    }
                }
            }
            n += 1;
        }
        n
    }

    /// read_string(): copy string contents; returns (bytes, new pos)
    pub fn read_string(&mut self, mut s: usize, delim: u8) -> (Vec<u8>, usize) {
        let mut out = Vec::new();
        if self.line[s] == delim {
            s += 1;
        }
        while self.line[s] != 0 {
            let c;
            if self.line[s] == b'\\' {
                let (ns, code) = self.escape(s);
                s = ns;
                c = code;
            } else {
                c = self.line[s];
                s += 1;
                if c == delim {
                    if self.line[s] == delim {
                        s += 1;
                    } else {
                        break;
                    }
                }
            }
            out.push(c);
        }
        (out, s)
    }

    /// parse_string(): None when the string is exactly one character (caller
    /// then uses the expression parser). Returns (data, new pos).
    pub fn parse_string(&mut self, s: usize, delim: u8) -> Option<(Vec<u8>, usize)> {
        // first pass validates and counts
        let _ = self.skip_string(s, delim);
        let size = self.string_size(s, delim);
        if size == 1 {
            return None;
        }
        Some(self.read_string(s, delim))
    }

    /// parse_name(): quoted, <...>, or bare name. Returns (name, new pos)
    pub fn parse_name(&mut self, mut s: usize) -> Option<(String, usize)> {
        let c = self.line[s];
        if c == b'"' || c == b'\'' {
            s += 1;
            let start = s;
            while self.line[s] != 0 && self.line[s] != c {
                s += 1;
            }
            let name = bytes_to_string(&self.line[start..s]);
            if self.line[s] != 0 {
                s = skip(&self.line, s + 1);
            }
            Some((name, s))
        } else if c == b'<' {
            s += 1;
            let start = s;
            while self.line[s] != 0 && self.line[s] != b'>' {
                s += 1;
            }
            let name = bytes_to_string(&self.line[start..s]);
            if self.line[s] != 0 {
                s = skip(&self.line, s + 1);
            }
            Some((name, s))
        } else {
            let start = s;
            while !is_eol(&self.line, s) && !is_space(self.line[s]) && self.line[s] != b',' {
                s += 1;
            }
            if s != start {
                let name = bytes_to_string(&self.line[start..s]);
                Some((name, skip(&self.line, s)))
            } else {
                None
            }
        }
    }
}
