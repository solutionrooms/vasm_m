//! Character classes and small scanning helpers from vasm's mot syntax module
//! (syntax/mot/syntax.h, syntax.c) and parse.c. All operate on byte slices that
//! are guaranteed to be NUL-terminated (index of the terminator is valid).

#[inline]
pub fn is_space(c: u8) -> bool {
    c == b' ' || (b'\t'..=b'\r').contains(&c)
}
#[inline]
pub fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}
#[inline]
pub fn is_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}
#[inline]
pub fn is_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}
#[inline]
pub fn to_lower(c: u8) -> u8 {
    c.to_ascii_lowercase()
}

/// ISIDSTART(x): '.' '@' '_' or alpha
#[inline]
pub fn is_id_start(c: u8) -> bool {
    c == b'.' || c == b'@' || c == b'_' || is_alpha(c)
}

/// isidchar(): alnum '_' '$' '%' (+ '.' when dot_idchar)
#[inline]
pub fn is_id_char(c: u8, dot_idchar: bool) -> bool {
    is_alnum(c) || c == b'_' || c == b'$' || c == b'%' || (dot_idchar && c == b'.')
}

/// ISBADID(p,l): single-character '.', '@' or '_'
#[inline]
pub fn is_bad_id(s: &[u8]) -> bool {
    s.len() == 1 && (s[0] == b'.' || s[0] == b'@' || s[0] == b'_')
}

/// skip(): advance over whitespace
#[inline]
pub fn skip(buf: &[u8], mut p: usize) -> usize {
    while is_space(buf[p]) {
        p += 1;
    }
    p
}

/// iscomment(): ';' anywhere (phxass '*'-after-blank rule not supported)
#[inline]
pub fn is_comment(buf: &[u8], p: usize) -> bool {
    buf[p] == b';'
}

/// ISEOL(p)
#[inline]
pub fn is_eol(buf: &[u8], p: usize) -> bool {
    buf[p] == 0 || is_comment(buf, p)
}

/// chkidend(): strip a trailing .b/.w/.l when dot_idchar is on
pub fn chk_id_end(buf: &[u8], start: usize, end: usize, dot_idchar: bool) -> usize {
    if dot_idchar && end - start > 2 && buf[end - 2] == b'.' {
        let c = to_lower(buf[end - 1]);
        if c == b'b' || c == b'w' || c == b'l' {
            return end - 2;
        }
    }
    end
}

/// skip_identifier(): returns end index of an identifier starting at p, or None
pub fn skip_identifier(buf: &[u8], p: usize, dot_idchar: bool) -> Option<usize> {
    if !is_id_start(buf[p]) {
        return None;
    }
    let mut s = p + 1;
    while is_id_char(buf[s], dot_idchar) {
        s += 1;
    }
    let e = chk_id_end(buf, p, s, dot_idchar);
    if is_bad_id(&buf[p..e]) { None } else { Some(e) }
}

/// cut_trail_blanks()/trim(): move end index back over trailing whitespace
pub fn trim_end(buf: &[u8], start: usize, mut end: usize) -> usize {
    while end > start && is_space(buf[end - 1]) {
        end -= 1;
    }
    end
}

/// Case-insensitive prefix compare of ASCII text (strnicmp == 0)
pub fn eq_nocase(buf: &[u8], p: usize, lit: &[u8]) -> bool {
    if p + lit.len() > buf.len() {
        return false;
    }
    buf[p..p + lit.len()].iter().zip(lit).all(|(a, b)| to_lower(*a) == to_lower(*b))
}

/// Case-insensitive slice equality
pub fn eq_nocase_slice(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| to_lower(*x) == to_lower(*y))
}

pub fn lower_string(s: &[u8]) -> String {
    String::from_utf8_lossy(s).to_ascii_lowercase()
}

pub fn bytes_to_string(s: &[u8]) -> String {
    // Source is treated as Latin-1/ASCII; keep bytes verbatim in a String
    s.iter().map(|&b| b as char).collect()
}
