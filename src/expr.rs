//! Expression trees, parser, simplifier and evaluator.
//! Transliterated from vasm 1.7h expr.c with the mot syntax module's
//! BOOLEAN(x) = -(x), exp_skip() and const_prefix() hooks.
use crate::asm::Assembler;
use crate::chars::*;
use crate::errors::Arg;
use crate::symbols::{SymKind, ABSLABEL, INEVAL, USED};
use crate::types::*;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add, Sub, Mul, Div, Mod, Neg, Cpl, LAnd, LOr, BAnd, BOr, Xor, Not, Lsh, Rsh, Rshu,
    Lt, Gt, Leq, Geq, Neq, Eq,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(Taddr),
    Huge(Thuge),
    Flt(f64),
    Sym(usize),
    Un(Op, Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    /// ((X op1 k1) op2 k2) ... with op in {Add, Sub} and constant k: evaluated
    /// left to right in the evaluator's own domain, so it is exactly the nested
    /// tree it replaces, but stays shallow when `SET` chains grow.
    Chain(Box<Expr>, Vec<(Op, Taddr)>),
}

impl Expr {
    pub fn depth(&self) -> usize {
        match self {
            Expr::Un(_, l) => 1 + l.depth(),
            Expr::Bin(_, l, r) => 1 + l.depth().max(r.depth()),
            Expr::Chain(x, _) => 1 + x.depth(),
            _ => 1,
        }
    }
    #[inline]
    pub fn is_const_leaf(&self) -> bool {
        matches!(self, Expr::Num(_) | Expr::Huge(_) | Expr::Flt(_))
    }
    /// type ordering NUM < HUG < FLT
    fn leaf_rank(&self) -> u8 {
        match self {
            Expr::Num(_) => 1,
            Expr::Huge(_) => 2,
            Expr::Flt(_) => 3,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpType {
    Num,
    Hug,
    Flt,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base {
    Illegal,
    Ok,
    PcRel,
    None,
}

/// Parser state (vasm's file-scope statics in expr.c)
pub struct ExprParser {
    make_tmp_lab: bool,
    exp_type: ExpType,
}

impl Assembler {
    // ----- exp_skip / EXPSKIP -------------------------------------------------
    /// exp_skip(): with -spaces skip blanks; otherwise a blank terminates the line.
    #[inline]
    pub fn exp_skip(&mut self, p: usize) -> usize {
        if self.opts.allow_spaces {
            skip(&self.line, p)
        } else {
            if is_space(self.line[p]) {
                self.line[p] = 0;
            }
            p
        }
    }

    // ----- entry points --------------------------------------------------------
    pub fn parse_expr(&mut self, p: &mut usize) -> Expr {
        self.parse_expr_mode(p, false, ExpType::Num)
    }
    pub fn parse_expr_tmplab(&mut self, p: &mut usize) -> Expr {
        self.parse_expr_mode(p, true, ExpType::Num)
    }
    pub fn parse_expr_huge(&mut self, p: &mut usize) -> Expr {
        self.parse_expr_mode(p, false, ExpType::Hug)
    }
    pub fn parse_expr_float(&mut self, p: &mut usize) -> Expr {
        self.parse_expr_mode(p, false, ExpType::Flt)
    }

    fn parse_expr_mode(&mut self, p: &mut usize, tmplab: bool, t: ExpType) -> Expr {
        let mut st = ExprParser { make_tmp_lab: tmplab, exp_type: t };
        let mut tree = self.expression(p, &mut st);
        self.simplify_expr(&mut tree);
        tree
    }

    /// parse_constexpr()
    pub fn parse_constexpr(&mut self, p: &mut usize) -> Taddr {
        let mut tree = self.parse_expr(p);
        self.simplify_expr(&mut tree);
        match tree {
            Expr::Num(v) => v,
            Expr::Huge(_) => { self.general_error(59, &[]); 0 }
            Expr::Flt(_) => { self.general_error(60, &[]); 0 }
            _ => { self.general_error(30, &[]); 0 }
        }
    }

    // ----- grammar -------------------------------------------------------------
    fn primary_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let c = self.line[*p];
        if c == b'(' {
            *p += 1;
            *p = self.exp_skip(*p);
            let e = self.expression(p, st);
            if self.line[*p] != b')' {
                self.general_error(6, &[Arg::C(b')')]);
            } else {
                *p += 1;
            }
            *p = self.exp_skip(*p);
            return e;
        }
        if let Some((name, np)) = self.get_local_label(*p) {
            *p = np;
            return self.sym_expr(&name);
        }
        // numbers: const_prefix()
        let (base, m): (u32, usize) = {
            let s = *p;
            let b = self.line[s];
            if is_digit(b) { (10, s) }
            else if b == b'$' { (16, s + 1) }
            else if b == b'@' && is_digit(self.line[s + 1]) { (8, s + 1) }
            else if b == b'%' { (2, s + 1) }
            else { (0, s) }
        };
        if base != 0 {
            return self.number(p, m, base, st);
        }
        if c == b'*' && !is_id_char(self.line[*p + 1], self.opts.local_dots) {
            *p += 1;
            *p = self.exp_skip(*p);
            if st.make_tmp_lab {
                let sym = self.new_tmplabel(None);
                self.add_atom(crate::atoms::Atom::label(sym));
                return Expr::Sym(sym);
            }
            let cpc = self.curpc_sym();
            return Expr::Sym(cpc);
        }
        if let Some(e) = skip_identifier(&self.line, *p, self.opts.local_dots) {
            let start = *p;
            *p = e;
            *p = self.exp_skip(*p);
            if let Some(sym) = self.symtab.find_bytes(&self.line[start..e]) {
                return self.sym_expr_idx(sym);
            }
            let name = bytes_to_string(&self.line[start..e]);
            if name == "NARG" {
                let n = self.cur_src.map(|s| self.sources[s].num_params).unwrap_or(-1);
                return Expr::Num(n);
            }
            return self.sym_expr(&name);
        }
        if c == b'\'' || c == b'"' {
            let quote = c;
            *p += 1;
            let mut val: Taddr = 0;
            let mut cnt = 0;
            loop {
                let b = self.line[*p];
                if b == 0 {
                    break;
                }
                let ch: u8;
                if b == b'\\' {
                    let (np, code) = self.escape(*p);
                    *p = np;
                    ch = code;
                } else {
                    *p += 1;
                    ch = b;
                    if ch == quote {
                        if self.line[*p] == quote {
                            *p += 1;
                        } else {
                            break;
                        }
                    }
                }
                cnt += 1;
                if cnt > BYTES_PER_TADDR {
                    self.general_error(21, &[Arg::from((BYTES_PER_TADDR * 8) as i64)]);
                    break;
                }
                val = val.wrapping_shl(8).wrapping_add(ch as Taddr);
            }
            *p = self.exp_skip(*p);
            return Expr::Num(val);
        }
        self.general_error(9, &[]);
        Expr::Num(-1)
    }

    fn sym_expr(&mut self, name: &str) -> Expr {
        let sym = match self.find_symbol(name) {
            Some(i) => i,
            None => self.new_import(name),
        };
        self.sym_expr_idx(sym)
    }

    fn sym_expr_idx(&mut self, sym: usize) -> Expr {
        self.symtab.syms[sym].flags |= USED;
        if self.symtab.syms[sym].kind != SymKind::Expression {
            Expr::Sym(sym)
        } else {
            // copy_tree(sym->expr)
            self.symtab.syms[sym].expr.as_ref().map(|e| (**e).clone()).unwrap_or(Expr::Num(0))
        }
    }

    fn number(&mut self, p: &mut usize, m: usize, base: u32, st: &mut ExprParser) -> Expr {
        let buf = &self.line;
        let mut s;
        let mut val: Utaddr = 0;
        let mut huge: Thuge = 0;
        let mut flt: f64 = 0.0;
        let mut ty = st.exp_type;
        'outer: loop {
            match ty {
                ExpType::Num => {
                    s = m;
                    val = 0;
                    if base <= 10 {
                        while buf[s] >= b'0' && buf[s] < b'0' + base as u8 {
                            let nval = (base as Utaddr).wrapping_mul(val);
                            if nval / base != val {
                                ty = ExpType::Hug;
                                continue 'outer;
                            }
                            val = nval.wrapping_add((buf[s] - b'0') as Utaddr);
                            s += 1;
                        }
                        if base == 10 && (buf[s] == b'e' || buf[s] == b'E' || (buf[s] == b'.' && is_digit(buf[s + 1]))) {
                            ty = ExpType::Flt;
                            continue 'outer;
                        }
                    } else if base == 16 {
                        loop {
                            let mut nval = val.wrapping_shl(4);
                            let b = buf[s];
                            if b.is_ascii_digit() { nval = nval.wrapping_add((b - b'0') as Utaddr); }
                            else if (b'a'..=b'f').contains(&b) { nval = nval.wrapping_add((b - b'a' + 10) as Utaddr); }
                            else if (b'A'..=b'F').contains(&b) { nval = nval.wrapping_add((b - b'A' + 10) as Utaddr); }
                            else { break; }
                            s += 1;
                            if nval >> 4 != val {
                                ty = ExpType::Hug;
                                continue 'outer;
                            }
                            val = nval;
                        }
                    }
                    break;
                }
                ExpType::Hug => {
                    s = m;
                    huge = 0;
                    if base <= 10 {
                        while buf[s] >= b'0' && buf[s] < b'0' + base as u8 {
                            huge = huge.wrapping_mul(base as i128).wrapping_add((buf[s] - b'0') as i128);
                            s += 1;
                        }
                        if base == 10 && (buf[s] == b'e' || buf[s] == b'E' || (buf[s] == b'.' && is_digit(buf[s + 1]))) {
                            ty = ExpType::Flt;
                            continue 'outer;
                        }
                    } else if base == 16 {
                        loop {
                            let b = buf[s];
                            let d = if b.is_ascii_digit() { b - b'0' }
                                else if (b'a'..=b'f').contains(&b) { b - b'a' + 10 }
                                else if (b'A'..=b'F').contains(&b) { b - b'A' + 10 }
                                else { break };
                            huge = huge.wrapping_mul(16).wrapping_add(d as i128);
                            s += 1;
                        }
                    }
                    break;
                }
                ExpType::Flt => {
                    if base != 10 {
                        ty = ExpType::Hug;
                        continue 'outer;
                    }
                    s = m;
                    // strtotfloat: parse the longest valid float prefix
                    let mut e = s;
                    while is_digit(buf[e]) { e += 1; }
                    if buf[e] == b'.' { e += 1; while is_digit(buf[e]) { e += 1; } }
                    if buf[e] == b'e' || buf[e] == b'E' {
                        let mut f = e + 1;
                        if buf[f] == b'+' || buf[f] == b'-' { f += 1; }
                        if is_digit(buf[f]) { while is_digit(buf[f]) { f += 1; } e = f; }
                    }
                    flt = bytes_to_string(&buf[s..e]).parse::<f64>().unwrap_or(0.0);
                    s = e;
                    break;
                }
            }
        }
        *p = s;
        *p = self.exp_skip(*p);
        st.exp_type = ty;
        match ty {
            ExpType::Num => Expr::Num(val as Taddr),
            ExpType::Hug => Expr::Huge(huge),
            ExpType::Flt => Expr::Flt(flt),
        }
    }

    fn unary_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let c = self.line[*p];
        if c == b'+' || c == b'-' || c == b'!' || c == b'~' {
            *p += 1;
            *p = self.exp_skip(*p);
            if c == b'+' {
                return self.primary_expr(p, st);
            }
            let op = match c { b'-' => Op::Neg, b'!' => Op::Not, _ => Op::Cpl };
            let e = self.primary_expr(p, st);
            return Expr::Un(op, Box::new(e));
        }
        self.primary_expr(p, st)
    }

    fn shift_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.unary_expr(p, st);
        loop {
            let c = self.line[*p];
            if !((c == b'<' || c == b'>') && self.line[*p + 1] == c) {
                break;
            }
            let op = if c == b'<' { Op::Lsh } else if self.unsigned_shift { Op::Rshu } else { Op::Rsh };
            *p += 2;
            *p = self.exp_skip(*p);
            let right = self.unary_expr(p, st);
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn and_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.shift_expr(p, st);
        while self.line[*p] == b'&' && self.line[*p + 1] != b'&' {
            *p += 1;
            *p = self.exp_skip(*p);
            let right = self.shift_expr(p, st);
            left = Expr::Bin(Op::BAnd, Box::new(left), Box::new(right));
        }
        left
    }

    fn exclusive_or_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.and_expr(p, st);
        while self.line[*p] == b'^' || self.line[*p] == b'~' {
            *p += 1;
            *p = self.exp_skip(*p);
            let right = self.and_expr(p, st);
            left = Expr::Bin(Op::Xor, Box::new(left), Box::new(right));
        }
        left
    }

    fn inclusive_or_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.exclusive_or_expr(p, st);
        loop {
            let c = self.line[*p];
            let n = self.line[*p + 1];
            if !((c == b'|' && n != b'|') || (c == b'!' && n != b'=')) {
                break;
            }
            *p += 1;
            *p = self.exp_skip(*p);
            let right = self.exclusive_or_expr(p, st);
            left = Expr::Bin(Op::BOr, Box::new(left), Box::new(right));
        }
        left
    }

    fn multiplicative_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.inclusive_or_expr(p, st);
        loop {
            let c = self.line[*p];
            if !(c == b'*' || c == b'/' || c == b'%') {
                break;
            }
            *p += 1;
            let op = if c == b'*' { Op::Mul }
                else if c == b'/' {
                    if self.line[*p] == b'/' { *p += 1; Op::Mod } else { Op::Div }
                } else { Op::Mod };
            *p = self.exp_skip(*p);
            let right = self.inclusive_or_expr(p, st);
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn additive_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.multiplicative_expr(p, st);
        loop {
            let c = self.line[*p];
            let n = self.line[*p + 1];
            if !((c == b'+' && n != b'+') || (c == b'-' && n != b'-')) {
                break;
            }
            *p += 1;
            *p = self.exp_skip(*p);
            let op = if c == b'+' { Op::Add } else { Op::Sub };
            let right = self.multiplicative_expr(p, st);
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn relational_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.additive_expr(p, st);
        loop {
            let c = self.line[*p];
            let n = self.line[*p + 1];
            if !(((c == b'<' && n != b'>') || c == b'>') && n != c) {
                break;
            }
            *p += 1;
            let op = if c == b'<' {
                if self.line[*p] == b'=' { *p += 1; Op::Leq } else { Op::Lt }
            } else if self.line[*p] == b'=' { *p += 1; Op::Geq } else { Op::Gt };
            *p = self.exp_skip(*p);
            let right = self.additive_expr(p, st);
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn equality_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.relational_expr(p, st);
        loop {
            let c = self.line[*p];
            let n = self.line[*p + 1];
            if !(c == b'=' || (c == b'!' && n == b'=') || (c == b'<' && n == b'>')) {
                break;
            }
            let m = c;
            *p += 1;
            let op = if m == b'=' { Op::Eq } else { Op::Neq };
            // if(m==*s||m!='=') s++;
            if m == self.line[*p] || m != b'=' {
                *p += 1;
            }
            *p = self.exp_skip(*p);
            let right = self.relational_expr(p, st);
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn logical_and_expr(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.equality_expr(p, st);
        while self.line[*p] == b'&' && self.line[*p + 1] == b'&' {
            *p += 2;
            *p = self.exp_skip(*p);
            let right = self.equality_expr(p, st);
            left = Expr::Bin(Op::LAnd, Box::new(left), Box::new(right));
        }
        left
    }

    fn expression(&mut self, p: &mut usize, st: &mut ExprParser) -> Expr {
        let mut left = self.logical_and_expr(p, st);
        while self.line[*p] == b'|' && self.line[*p + 1] == b'|' {
            *p += 2;
            *p = self.exp_skip(*p);
            let right = self.logical_and_expr(p, st);
            left = Expr::Bin(Op::LOr, Box::new(left), Box::new(right));
        }
        left
    }

    // ----- simplify ------------------------------------------------------------
    pub fn simplify_expr(&mut self, tree: &mut Expr) {
        match tree {
            Expr::Un(_, l) => self.simplify_expr(l),
            Expr::Bin(_, l, r) => {
                self.simplify_expr(l);
                self.simplify_expr(r);
            }
            Expr::Chain(x, _) => self.simplify_expr(x),
            _ => {}
        }
        // Rearrange "const - sym - sym" so that "sym - sym" is evaluated first
        if let Expr::Bin(Op::Sub, l, r) = tree {
            if matches!(**r, Expr::Sym(_)) {
                if let Expr::Bin(Op::Sub, ll, lr) = &mut **l {
                    if !matches!(**ll, Expr::Sym(_)) && matches!(**lr, Expr::Sym(_)) {
                        let x = std::mem::replace(&mut **r, Expr::Num(0));
                        let a = std::mem::replace(&mut **ll, Expr::Num(0));
                        let b = std::mem::replace(&mut **lr, Expr::Num(0));
                        // tree = a - (b - x)
                        *tree = Expr::Bin(Op::Sub, Box::new(a), Box::new(Expr::Bin(Op::Sub, Box::new(b), Box::new(x))));
                    }
                }
            }
        }
        // Turn ((X +/- a) +/- b) with constant a, b into a Chain node. The
        // constants are NOT combined here: they are applied one by one at
        // evaluation time in the evaluator's domain (i32 wrapping, i128 or f64),
        // exactly like the nested tree, so a forward symbol resolving to a
        // float or huge value evaluates identically.
        if let Expr::Bin(op2 @ (Op::Add | Op::Sub), l, r) = tree {
            if let Expr::Num(b) = **r {
                let (op2, b) = (*op2, b);
                match &mut **l {
                    Expr::Chain(_, v) => {
                        v.push((op2, b));
                        let inner = std::mem::replace(&mut **l, Expr::Num(0));
                        *tree = inner;
                    }
                    Expr::Bin(op1 @ (Op::Add | Op::Sub), il, ir) => {
                        if let Expr::Num(a) = **ir {
                            let op1 = *op1;
                            let x = std::mem::replace(&mut **il, Expr::Num(0));
                            *tree = Expr::Chain(Box::new(x), vec![(op1, a), (op2, b)]);
                        }
                    }
                    _ => {}
                }
            }
        }
        // A chain whose base folded to a constant folds sequentially in that
        // constant's domain (same order as the nested tree would fold).
        if let Expr::Chain(x, v) = tree {
            if x.is_const_leaf() {
                let mut acc = (**x).clone();
                for (op, k) in v.iter() {
                    acc = match self.fold(*op, &acc, Some(&Expr::Num(*k))) {
                        Some(f) => f,
                        None => break,
                    };
                }
                if acc.is_const_leaf() {
                    *tree = acc;
                }
            }
        }
        let folded: Option<Expr> = match tree {
            Expr::Sym(s) => {
                let sym = &self.symtab.syms[*s];
                if sym.kind == SymKind::Expression {
                    match sym.expr.as_deref() {
                        Some(e @ (Expr::Num(_) | Expr::Huge(_) | Expr::Flt(_))) => Some(e.clone()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Expr::Un(op, l) => {
                if !l.is_const_leaf() { None } else { self.fold(*op, l, None) }
            }
            Expr::Bin(op, l, r) => {
                if !l.is_const_leaf() || !r.is_const_leaf() { None } else { self.fold(*op, l, Some(r)) }
            }
            _ => None,
        };
        if let Some(f) = folded {
            *tree = f;
        }
    }

    fn fold(&mut self, op: Op, l: &Expr, r: Option<&Expr>) -> Option<Expr> {
        let rank = l.leaf_rank().max(r.map(|x| x.leaf_rank()).unwrap_or(0));
        match rank {
            1 => {
                let lv = if let Expr::Num(v) = l { *v } else { 0 };
                let rv = if let Some(Expr::Num(v)) = r { *v } else { 0 };
                self.num_op(op, lv, rv).map(Expr::Num)
            }
            2 => {
                let to_h = |e: &Expr| match e { Expr::Num(v) => *v as i128, Expr::Huge(h) => *h, _ => 0 };
                let lv = to_h(l);
                let rv = r.map(to_h).unwrap_or(0);
                self.huge_op(op, lv, rv)
            }
            3 => {
                let to_f = |e: &Expr| match e { Expr::Num(v) => *v as f64, Expr::Huge(h) => *h as f64, Expr::Flt(f) => *f, _ => 0.0 };
                let lv = to_f(l);
                let rv = r.map(to_f).unwrap_or(0.0);
                let f = match op {
                    Op::Add => lv + rv,
                    Op::Sub => lv - rv,
                    Op::Mul => lv * rv,
                    Op::Div => { if rv == 0.0 { self.general_error(41, &[]); 0.0 } else { lv / rv } }
                    Op::Neg => -lv,
                    Op::Lt => return Some(Expr::Num(boolean(lv < rv))),
                    Op::Gt => return Some(Expr::Num(boolean(lv > rv))),
                    Op::Leq => return Some(Expr::Num(boolean(lv <= rv))),
                    Op::Geq => return Some(Expr::Num(boolean(lv >= rv))),
                    Op::Neq => return Some(Expr::Num(boolean(lv != rv))),
                    Op::Eq => return Some(Expr::Num(boolean(lv == rv))),
                    _ => return None,
                };
                Some(Expr::Flt(f))
            }
            _ => None,
        }
    }

    /// Integer (taddr = int32, wrapping) operator semantics shared by
    /// simplify_expr and eval_expr.
    pub fn num_op(&mut self, op: Op, l: Taddr, r: Taddr) -> Option<Taddr> {
        Some(match op {
            Op::Add => l.wrapping_add(r),
            Op::Sub => l.wrapping_sub(r),
            Op::Mul => l.wrapping_mul(r),
            Op::Div => { if r == 0 { self.general_error(41, &[]); 0 } else { l.wrapping_div(r) } }
            Op::Mod => { if r == 0 { self.general_error(41, &[]); 0 } else { l.wrapping_rem(r) } }
            Op::Neg => l.wrapping_neg(),
            Op::Cpl => !l,
            Op::LAnd => boolean(l != 0 && r != 0),
            Op::LOr => boolean(l != 0 || r != 0),
            Op::BAnd => l & r,
            Op::BOr => l | r,
            Op::Xor => l ^ r,
            Op::Not => (l == 0) as Taddr,
            Op::Lsh => c_shl(l, r),
            Op::Rsh => c_shr(l, r),
            Op::Rshu => c_shru(l, r),
            Op::Lt => boolean(l < r),
            Op::Gt => boolean(l > r),
            Op::Leq => boolean(l <= r),
            Op::Geq => boolean(l >= r),
            Op::Neq => boolean(l != r),
            Op::Eq => boolean(l == r),
        })
    }

    fn huge_op(&mut self, op: Op, l: Thuge, r: Thuge) -> Option<Expr> {
        let h = |v: Thuge| Some(Expr::Huge(v));
        let n = |v: Taddr| Some(Expr::Num(v));
        match op {
            Op::Add => h(l.wrapping_add(r)),
            Op::Sub => h(l.wrapping_sub(r)),
            Op::Mul => h(l.wrapping_mul(r)),
            Op::Div => { if r == 0 { self.general_error(41, &[]); h(0) } else { h(l.wrapping_div(r)) } }
            Op::Mod => { if r == 0 { self.general_error(41, &[]); h(0) } else { h(l.wrapping_rem(r)) } }
            Op::Neg => h(l.wrapping_neg()),
            Op::Cpl => h(!l),
            Op::LAnd => n(boolean(l != 0 && r != 0)),
            Op::LOr => n(boolean(l != 0 || r != 0)),
            Op::BAnd => h(l & r),
            Op::BOr => h(l | r),
            Op::Xor => h(l ^ r),
            Op::Not => h((l == 0) as i128),
            Op::Lsh => h(if huge_to_int(r) as u32 >= 128 { 0 } else { l << (huge_to_int(r) as u32) }),
            Op::Rsh => h(if huge_to_int(r) as u32 >= 128 { if l < 0 { -1 } else { 0 } } else { l >> (huge_to_int(r) as u32) }),
            Op::Rshu => h(if huge_to_int(r) as u32 >= 128 { 0 } else { ((l as u128) >> (huge_to_int(r) as u32)) as i128 }),
            Op::Lt => n(boolean(l < r)),
            Op::Gt => n(boolean(l > r)),
            Op::Leq => n(boolean(l <= r)),
            Op::Geq => n(boolean(l >= r)),
            Op::Neq => n(boolean(l != r)),
            Op::Eq => n(boolean(l == r)),
        }
    }

    // ----- evaluate ------------------------------------------------------------
    /// eval_expr(): returns (value, is_constant). Value is always produced.
    pub fn eval_expr(&mut self, tree: &Expr, sec: Option<usize>, pc: Taddr) -> (Taddr, bool) {
        match tree {
            Expr::Num(v) => (*v, true),
            Expr::Huge(h) => {
                if !huge_chkrange(*h, 32) {
                    self.general_error(21, &[Arg::from(32i64)]);
                }
                (huge_to_int(*h) as Taddr, true)
            }
            Expr::Flt(f) => {
                if !flt_chkrange(*f, 32) {
                    self.general_error(21, &[Arg::from(32i64)]);
                }
                (*f as i64 as Taddr, true)
            }
            Expr::Sym(s) => {
                let s = *s;
                let sym = &self.symtab.syms[s];
                match sym.kind {
                    SymKind::Expression => {
                        if sym.flags & INEVAL != 0 {
                            let name = sym.name.clone();
                            self.general_error(18, &[Arg::from(name)]);
                        }
                        self.symtab.syms[s].flags |= INEVAL;
                        let e = self.symtab.syms[s].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                        let r = self.eval_expr(&e, sec, pc);
                        self.symtab.syms[s].flags &= !INEVAL;
                        r
                    }
                    SymKind::LabSym if sym.flags & crate::symbols::WEAK == 0 => {
                        self.update_curpc(s, sec, pc);
                        let sym = &self.symtab.syms[s];
                        let cnst = match sym.sec {
                            None => false,
                            Some(ss) => self.sections[ss].flags & crate::atoms::UNALLOCATED != 0,
                        };
                        (sym.pc, cnst)
                    }
                    _ => (0, false),
                }
            }
            Expr::Un(op, l) => {
                let (lv, lc) = self.eval_expr(l, sec, pc);
                (self.num_op(*op, lv, 0).unwrap_or(0), lc)
            }
            Expr::Chain(x, v) => {
                let (mut val, cnst) = self.eval_expr(x, sec, pc);
                for (op, k) in v.iter() {
                    val = if *op == Op::Add { val.wrapping_add(*k) } else { val.wrapping_sub(*k) };
                }
                (val, cnst)
            }
            Expr::Bin(op, l, r) => {
                let (lv, lc) = self.eval_expr(l, sec, pc);
                let (rv, rc) = self.eval_expr(r, sec, pc);
                let mut cnst = lc && rc;
                if *op == Op::Sub {
                    let (_, lsym) = self.find_base(l, sec, pc);
                    let (_, rsym) = self.find_base(r, sec, pc);
                    if let (false, Some(ls), Some(rs)) = (cnst, lsym, rsym) {
                        if self.symtab.syms[rs].is_locref() {
                            let lsy = &self.symtab.syms[ls];
                            let rsy = &self.symtab.syms[rs];
                            if lsy.is_locref() && lsy.sec == rsy.sec {
                                cnst = true;
                            } else if rsy.sec == sec && (lsy.is_extref() || lsy.is_locref()) {
                                if rsy.flags & ABSLABEL != 0 && lsy.flags & ABSLABEL != 0 {
                                    cnst = true;
                                } else {
                                    let lorg = lsy.sec.map(|x| self.sections[x].org).unwrap_or(0);
                                    let val = pc.wrapping_sub(rv).wrapping_add(lv).wrapping_sub(lorg);
                                    return (val, cnst);
                                }
                            }
                        }
                    }
                    return (lv.wrapping_sub(rv), cnst);
                }
                (self.num_op(*op, lv, rv).unwrap_or(0), cnst)
            }
        }
    }

    /// eval_expr_huge(): (value, ok)
    pub fn eval_expr_huge(&mut self, tree: &Expr) -> Option<Thuge> {
        match tree {
            Expr::Num(v) => Some(*v as i128),
            Expr::Huge(h) => Some(*h),
            Expr::Flt(f) => Some(*f as i128),
            Expr::Sym(s) => {
                let s = *s;
                if self.symtab.syms[s].kind == SymKind::Expression {
                    if self.symtab.syms[s].flags & INEVAL != 0 {
                        let name = self.symtab.syms[s].name.clone();
                        self.general_error(18, &[Arg::from(name)]);
                    }
                    self.symtab.syms[s].flags |= INEVAL;
                    let e = self.symtab.syms[s].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                    let r = self.eval_expr_huge(&e);
                    self.symtab.syms[s].flags &= !INEVAL;
                    r
                } else {
                    None
                }
            }
            Expr::Un(op, l) => {
                let lv = self.eval_expr_huge(l)?;
                match self.huge_op(*op, lv, 0)? {
                    Expr::Huge(h) => Some(h),
                    Expr::Num(n) => Some(n as i128),
                    _ => None,
                }
            }
            Expr::Chain(x, v) => {
                let mut val = self.eval_expr_huge(x)?;
                for (op, k) in v.iter() {
                    val = if *op == Op::Add { val.wrapping_add(*k as i128) } else { val.wrapping_sub(*k as i128) };
                }
                Some(val)
            }
            Expr::Bin(op, l, r) => {
                let lv = self.eval_expr_huge(l)?;
                let rv = self.eval_expr_huge(r)?;
                match self.huge_op(*op, lv, rv)? {
                    Expr::Huge(h) => Some(h),
                    Expr::Num(n) => Some(n as i128),
                    _ => None,
                }
            }
        }
    }

    /// eval_expr_float()
    pub fn eval_expr_float(&mut self, tree: &Expr) -> Option<f64> {
        match tree {
            Expr::Num(v) => Some(*v as f64),
            Expr::Huge(h) => Some(*h as f64),
            Expr::Flt(f) => Some(*f),
            Expr::Sym(s) => {
                let s = *s;
                if self.symtab.syms[s].kind == SymKind::Expression {
                    self.symtab.syms[s].flags |= INEVAL;
                    let e = self.symtab.syms[s].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                    let r = self.eval_expr_float(&e);
                    self.symtab.syms[s].flags &= !INEVAL;
                    r
                } else {
                    None
                }
            }
            Expr::Un(op, l) => {
                let lv = self.eval_expr_float(l)?;
                match op { Op::Neg => Some(-lv), _ => None }
            }
            Expr::Chain(x, v) => {
                let mut val = self.eval_expr_float(x)?;
                for (op, k) in v.iter() {
                    val = if *op == Op::Add { val + *k as f64 } else { val - *k as f64 };
                }
                Some(val)
            }
            Expr::Bin(op, l, r) => {
                let lv = self.eval_expr_float(l)?;
                let rv = self.eval_expr_float(r)?;
                Some(match op {
                    Op::Add => lv + rv,
                    Op::Sub => lv - rv,
                    Op::Mul => lv * rv,
                    Op::Div => { if rv == 0.0 { self.general_error(41, &[]); 0.0 } else { lv / rv } }
                    Op::Lt => boolean(lv < rv) as f64,
                    Op::Gt => boolean(lv > rv) as f64,
                    Op::Leq => boolean(lv <= rv) as f64,
                    Op::Geq => boolean(lv >= rv) as f64,
                    Op::Neq => boolean(lv != rv) as f64,
                    Op::Eq => boolean(lv == rv) as f64,
                    _ => return None,
                })
            }
        }
    }

    /// type_of_expr(): 1=NUM 2=HUG 3=FLT (0 = none)
    pub fn type_of_expr(&mut self, tree: &Expr) -> u8 {
        match tree {
            Expr::Num(_) => 1,
            Expr::Huge(_) => 2,
            Expr::Flt(_) => 3,
            Expr::Sym(s) => {
                let s = *s;
                let (flags, kind) = (self.symtab.syms[s].flags, self.symtab.syms[s].kind);
                if flags & INEVAL != 0 {
                    let name = self.symtab.syms[s].name.clone();
                    self.general_error(18, &[Arg::from(name)]);
                }
                if kind == SymKind::Expression {
                    self.symtab.syms[s].flags |= INEVAL;
                    let e = self.symtab.syms[s].expr.clone();
                    let t = e.as_deref().map(|e| self.type_of_expr(e)).unwrap_or(1);
                    self.symtab.syms[s].flags &= !INEVAL;
                    t
                } else {
                    1
                }
            }
            Expr::Un(_, l) => self.type_of_expr(l),
            Expr::Chain(x, _) => self.type_of_expr(x),
            Expr::Bin(_, l, r) => {
                let a = self.type_of_expr(l);
                let b = self.type_of_expr(r);
                a.max(b)
            }
        }
    }

    // ----- find_base -----------------------------------------------------------
    /// find_abs_base(): Ok(base) or Err(()) for "not ok"
    fn find_abs_base(&mut self, tree: &Expr, base: &mut Option<usize>) -> bool {
        match tree {
            Expr::Sym(s) => {
                let s = *s;
                let sym = &self.symtab.syms[s];
                match sym.kind {
                    SymKind::Expression => {
                        if sym.flags & INEVAL != 0 { return false; }
                        self.symtab.syms[s].flags |= INEVAL;
                        let e = self.symtab.syms[s].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                        let ok = self.find_abs_base(&e, base);
                        self.symtab.syms[s].flags &= !INEVAL;
                        ok
                    }
                    _ if sym.is_locref() => {
                        if sym.flags & ABSLABEL != 0 {
                            *base = Some(s);
                            return true;
                        }
                        false
                    }
                    _ => {
                        if let Some(cs) = self.current_section {
                            if self.sections[cs].flags & crate::atoms::ABSOLUTE != 0 {
                                *base = Some(s);
                                return true;
                            }
                        }
                        false
                    }
                }
            }
            Expr::Num(_) | Expr::Huge(_) | Expr::Flt(_) => true,
            Expr::Un(_, l) | Expr::Chain(l, _) => {
                if !self.find_abs_base(l, base) { return false; }
                true
            }
            Expr::Bin(_, l, r) => {
                if !self.find_abs_base(l, base) { return false; }
                if base.is_some() {
                    let mut tst: Option<usize> = None;
                    if !self.find_abs_base(r, &mut tst) { return false; }
                    if tst.is_some() {
                        *base = None;
                        return true;
                    }
                } else if !self.find_abs_base(r, base) {
                    return false;
                }
                true
            }
        }
    }

    fn find_base_inner(&mut self, p: &Expr, base: &mut Option<usize>, sec: Option<usize>, pc: Taddr) -> Base {
        match p {
            Expr::Sym(s) => {
                let s = *s;
                self.update_curpc(s, sec, pc);
                if self.symtab.syms[s].kind == SymKind::Expression {
                    if self.symtab.syms[s].flags & INEVAL != 0 {
                        return Base::Illegal;
                    }
                    let e = self.symtab.syms[s].expr.clone().unwrap_or_else(|| Rc::new(Expr::Num(0)));
                    self.symtab.syms[s].flags |= INEVAL;
                    let r = self.find_base_inner(&e, base, sec, pc);
                    self.symtab.syms[s].flags &= !INEVAL;
                    return r;
                }
                *base = Some(s);
                Base::Ok
            }
            Expr::Chain(x, _) => {
                // (X op k) with constant k: BASE_OK iff X has a base
                if self.find_base_inner(x, base, sec, pc) == Base::Ok { Base::Ok } else { Base::Illegal }
            }
            Expr::Bin(Op::Add, l, r) => {
                if self.eval_expr(l, sec, pc).1 && self.find_base_inner(r, base, sec, pc) == Base::Ok {
                    return Base::Ok;
                }
                if self.eval_expr(r, sec, pc).1 && self.find_base_inner(l, base, sec, pc) == Base::Ok {
                    return Base::Ok;
                }
                Base::Illegal
            }
            Expr::Bin(Op::Sub, l, r) => {
                if self.eval_expr(r, sec, pc).1 && self.find_base_inner(l, base, sec, pc) == Base::Ok {
                    return Base::Ok;
                }
                let mut pcsym: Option<usize> = None;
                if self.find_base_inner(l, base, sec, pc) == Base::Ok && self.find_base_inner(r, &mut pcsym, sec, pc) == Base::Ok {
                    let ps = &self.symtab.syms[pcsym.unwrap()];
                    let bs = &self.symtab.syms[base.unwrap()];
                    if ps.is_locref() && ps.sec == sec && (bs.is_locref() || bs.is_extref()) {
                        return Base::PcRel;
                    }
                }
                Base::Illegal
            }
            _ => Base::Illegal,
        }
    }

    /// find_base(): returns (kind, base symbol)
    pub fn find_base(&mut self, p: &Expr, sec: Option<usize>, pc: Taddr) -> (Base, Option<usize>) {
        let mut base: Option<usize> = None;
        if self.find_abs_base(p, &mut base) {
            if let Some(b) = base {
                return (if self.symtab.syms[b].is_extref() { Base::Illegal } else { Base::Ok }, base);
            }
            return (Base::None, None);
        }
        let mut base: Option<usize> = None;
        let k = self.find_base_inner(p, &mut base, sec, pc);
        (k, base)
    }
}

/// C semantics for int32 shifts as compiled on typical targets: shift count
/// masked to 5 bits (x86/arm64 behaviour), which is what the reference binary does.
#[inline]
pub fn c_shl(l: Taddr, r: Taddr) -> Taddr {
    l.wrapping_shl(r as u32)
}
#[inline]
pub fn c_shr(l: Taddr, r: Taddr) -> Taddr {
    l.wrapping_shr(r as u32)
}
#[inline]
pub fn c_shru(l: Taddr, r: Taddr) -> Taddr {
    ((l as Utaddr).wrapping_shr(r as u32)) as Taddr
}
