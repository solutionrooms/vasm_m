//! Basic scalar types mirroring vasm's cpus/m68k/cpu.h.
pub type Taddr = i32;
pub type Utaddr = u32;
pub const BYTES_PER_TADDR: usize = 4;
pub const INST_ALIGN: Taddr = 2;
pub const MAX_OPERANDS: usize = 6;
pub const MAX_QUALIFIERS: usize = 1;

#[inline]
pub fn data_align(bits: i32) -> Taddr {
    if bits <= 8 { 1 } else { 2 }
}

/// BOOLEAN(x) in the mot syntax module is -(x): true is -1.
#[inline]
pub fn boolean(b: bool) -> Taddr {
    if b { -1 } else { 0 }
}

/// 128-bit "thuge" value (hi:lo) as in hugeint.c. We use i128 arithmetic.
pub type Thuge = i128;

/// hugeint.c huge_chkrange for bits < 64.
pub fn huge_chkrange(h: Thuge, bits: u32) -> bool {
    let lo = h as u64;
    let hi = (h >> 64) as u64;
    if bits >= 128 {
        return true;
    }
    if bits >= 64 {
        let mask: u64 = !0u64 << (bits - 64);
        let v = hi & mask;
        return if v & (1u64 << (bits - 64)) != 0 { (v ^ mask) == 0 } else { v == 0 };
    }
    let mask: u64 = !0u64 << bits;
    let v = lo & mask;
    if v & (1u64 << bits) != 0 {
        hi == !0u64 && (v ^ mask) == 0
    } else {
        hi == 0 && v == 0
    }
}

#[inline]
pub fn huge_to_int(h: Thuge) -> i64 {
    h as u64 as i64
}

pub fn flt_chkrange(f: f64, bits: u32) -> bool {
    // tfloat.h flt_chkrange: check whether the float fits into bits as integer
    let max = (1u128 << bits) as f64;
    let min = -((1u128 << (bits - 1)) as f64);
    f < max && f >= min
}
