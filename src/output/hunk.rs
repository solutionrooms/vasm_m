//! -Fhunk / -Fhunkexe output modules, transliterated from vasm 1.7h
//! output_hunk.c ("vasm hunk format output module 2.9") and output_hunk.h.
use crate::asm::Assembler;
use crate::atoms::*;
use crate::errors::Arg;
use crate::symbols::{SymKind, COMMON, EXPORT, UNLISTED, WEAK};
use crate::types::Taddr;

const HUNK_UNIT: u32 = 999;
const HUNK_NAME: u32 = 1000;
const HUNK_CODE: u32 = 1001;
const HUNK_DATA: u32 = 1002;
const HUNK_BSS: u32 = 1003;
const HUNK_ABSRELOC32: u32 = 1004;
const HUNK_RELRELOC16: u32 = 1005;
const HUNK_RELRELOC8: u32 = 1006;
const HUNK_EXT: u32 = 1007;
const HUNK_SYMBOL: u32 = 1008;
const HUNK_DEBUG: u32 = 1009;
const HUNK_END: u32 = 1010;
const HUNK_HEADER: u32 = 1011;
const HUNK_DREL32: u32 = 1015;
const HUNK_DREL16: u32 = 1016;
const HUNK_RELRELOC32: u32 = 1021;
const HUNK_RELRELOC26: u32 = 1260;

const HUNKF_CHIP: u32 = 1 << 30;
const HUNKF_FAST: u32 = 1 << 31;
const HUNKF_MEMTYPE: u32 = HUNKF_CHIP | HUNKF_FAST;
const MEMF_PUBLIC: u32 = 1 << 0;
const MEMF_CHIP: u32 = 1 << 1;
const MEMF_FAST: u32 = 1 << 2;

const EXT_SYMB: u32 = 0;
const EXT_DEF: u32 = 1;
const EXT_ABS: u32 = 2;
const EXT_ABSREF32: u32 = 129;
const EXT_ABSCOMMON: u32 = 130;
const EXT_RELREF16: u32 = 131;
const EXT_RELREF8: u32 = 132;
const EXT_DEXT16: u32 = 134;
const EXT_RELREF32: u32 = 136;
const EXT_RELCOMMON: u32 = 137;
const EXT_ABSREF16: u32 = 138;
const EXT_ABSREF8: u32 = 139;

const LAST_STANDARD_RELOC: i32 = 16;

struct HunkReloc {
    id: u32,
    index: u32,
    offset: u32,
}

struct HunkXref {
    name: String,
    ty: u32,
    size: u32,
    offset: u32,
}

struct HunkLine {
    line: u32,
    offset: u32,
}

/// Result of prepare_sections(): section indices (None = deleted or offset
/// section), per-index symbol lists in vasm's order, and the list head.
struct Prep {
    first_sec: Option<usize>,
    sec_cnt: u32,
    idx: Vec<Option<u32>>,
    secsyms: Vec<Vec<usize>>,
}

#[inline]
fn fw16(out: &mut Vec<u8>, x: u16) {
    out.extend_from_slice(&x.to_be_bytes());
}
#[inline]
fn fw32(out: &mut Vec<u8>, x: u32) {
    out.extend_from_slice(&x.to_be_bytes());
}
/// strlen32(): number of 32-bit words for a string without terminator
#[inline]
fn strlen32(s: &str) -> u32 {
    ((s.len() + 3) >> 2) as u32
}
/// fwalign(): pad with zeros up to the next multiple of align
fn fwalign(out: &mut Vec<u8>, n: u32, align: u32) {
    let pad = balign(n as Taddr, align as Taddr) as u32;
    out.resize(out.len() + pad as usize, 0);
}
fn fwname(out: &mut Vec<u8>, name: &str) {
    out.extend_from_slice(name.as_bytes());
    fwalign(out, name.len() as u32, 4);
}
fn fwsblock(out: &mut Vec<u8>, sb: &SBlock) {
    for _ in 0..sb.space {
        out.extend_from_slice(&sb.fill[..sb.size]);
    }
}

/// scan_attr(): hunk type from section attributes
fn scan_attr(attr: &str) -> u32 {
    if attr.is_empty() {
        return HUNK_DATA;
    }
    let mut ty = 0;
    for c in attr.chars() {
        match c {
            'c' => ty = HUNK_CODE,
            'd' => ty = HUNK_DATA,
            'u' => ty = HUNK_BSS,
            _ => {}
        }
    }
    ty
}

impl Assembler {
    fn get_sec_size(&self, sec: usize) -> u32 {
        (self.sections[sec].pc as u32).wrapping_sub(self.sections[sec].org as u32)
    }

    /// get_sym_value(): alignment for common symbols
    fn get_sym_value(&mut self, si: usize) -> Taddr {
        let s = &self.symtab.syms[si];
        if s.flags & COMMON != 0 {
            s.align
        } else if s.kind == SymKind::LabSym {
            s.pc
        } else if s.kind == SymKind::Expression {
            match s.expr.clone() {
                Some(e) => self.eval_expr(&e, None, 0).0,
                None => self.ierror(0, "hunk.rs", line!()),
            }
        } else {
            0
        }
    }

    /// get_sym_size()
    fn get_sym_size(&mut self, si: usize) -> Taddr {
        match self.symtab.syms[si].size.clone() {
            Some(e) => self.eval_expr(&e, None, 0).0,
            None => 0,
        }
    }

    /// fwmemflags(): memory attributes with data (section size or type)
    fn fwmemflags(&self, out: &mut Vec<u8>, sec: usize, data: u32) {
        let mem = self.sections[sec].memattr;
        if mem & !MEMF_PUBLIC == 0 {
            fw32(out, data);
        } else if mem & !MEMF_PUBLIC == MEMF_CHIP {
            fw32(out, HUNKF_CHIP | data);
        } else if mem & !MEMF_PUBLIC == MEMF_FAST {
            fw32(out, HUNKF_FAST | data);
        } else {
            fw32(out, HUNKF_MEMTYPE | data);
            fw32(out, mem);
        }
    }

    /// fwnopalign(): align a 68k code section with NOP instructions
    fn fwnopalign(&mut self, out: &mut Vec<u8>, n: u32) {
        let n = balign(n as Taddr, 4) as u32;
        if n & 1 != 0 {
            self.ierror(0, "hunk.rs", line!());
        }
        let mut i = 0;
        while i < n {
            fw16(out, 0x4e71);
            i += 2;
        }
    }

    fn dummy_section(&mut self) -> usize {
        self.new_section(".text", "acrx3", 8)
    }

    /// prepare_sections(): drop empty sections, assign symbols to sections,
    /// make sure every common symbol is referenced, guarantee one section
    /// when there are symbols.
    fn prepare_sections(&mut self) -> Prep {
        let keep_empty = self.opts.hunk_keepempty;
        let nsec = self.sections.len();
        let mut idx: Vec<Option<u32>> = vec![None; nsec];
        let mut sec_cnt: u32 = 0;
        let mut first_nonbss: Option<usize> = None;
        let mut comm_referenced = vec![false; self.symtab.syms.len()];
        // remove_unalloc_sects() already happened in vasm; offset sections are
        // not part of the list here.
        let list: Vec<usize> = (0..nsec).filter(|&s| self.sections[s].flags & UNALLOCATED == 0).collect();
        for &s in &list {
            // ignore empty sections without symbols, unless -keepempty
            if keep_empty || self.get_sec_size(s) != 0 || self.sections[s].flags & HAS_SYMBOLS != 0 {
                idx[s] = Some(sec_cnt);
                sec_cnt += 1;
            } else {
                continue;
            }
            if first_nonbss.is_none() && scan_attr(&self.sections[s].attr) != HUNK_BSS {
                first_nonbss = Some(s);
            }
            // flag all present common-symbol references from this section
            for a in &self.sections[s].atoms {
                if let AtomKind::Data(db) = &a.kind {
                    for r in &db.relocs {
                        if (r.kind == REL_ABS || r.kind == REL_PC) && r.size == 32 && self.symtab.syms[r.sym].flags & COMMON != 0 {
                            comm_referenced[r.sym] = true;
                        }
                    }
                }
            }
        }
        let mut first_sec: Option<usize> = list.first().copied();
        let mut secsyms: Vec<Vec<usize>> = vec![Vec::new(); sec_cnt as usize + 1];

        // vasm walks its symbol list newest-first and prepends to each
        // section's list, so the lists end up in creation order; side effects
        // (dummy atoms) happen newest-first.
        let nsyms = self.symtab.syms.len();
        for si in (0..nsyms).rev() {
            let name = self.symtab.syms[si].name.clone();
            if name.starts_with(' ') || self.symtab.syms[si].flags & UNLISTED != 0 {
                continue; // internal symbols are ignored; orphans are not in vasm's list
            }
            let flags = self.symtab.syms[si].flags;
            if flags & COMMON != 0 && !comm_referenced[si] {
                // create a dummy reference for each unreferenced common symbol
                let mut db = DBlock::new(vec![0u8; 4]);
                db.relocs.push(Reloc { kind: REL_ABS, byteoffset: 0, bitoffset: 0, size: 32, mask: -1, addend: 0, sym: si });
                if first_nonbss.is_none() {
                    let d = self.dummy_section();
                    first_nonbss = Some(d);
                    if first_sec.is_none() {
                        first_sec = Some(d);
                    }
                    idx.resize(self.sections.len(), None);
                    idx[d] = Some(sec_cnt);
                    sec_cnt += 1;
                    secsyms.push(Vec::new());
                }
                self.add_atom_to(first_nonbss, Atom::data(db, 4));
            } else if flags & WEAK != 0 {
                // weak symbols are not supported, make them global
                let s = &mut self.symtab.syms[si];
                s.flags &= !WEAK;
                s.flags |= EXPORT;
                self.output_error(10, &[Arg::from(name.clone())]);
            }
            let kind = self.symtab.syms[si].kind;
            let flags = self.symtab.syms[si].flags;
            if kind == SymKind::LabSym || (kind == SymKind::Expression && flags & EXPORT != 0) {
                if kind == SymKind::Expression {
                    // put absolute global symbols into the first section
                    if first_sec.is_none() {
                        let d = self.dummy_section();
                        first_sec = Some(d);
                        first_nonbss = Some(d);
                        idx.resize(self.sections.len(), None);
                        idx[d] = Some(sec_cnt);
                        sec_cnt += 1;
                        secsyms.push(Vec::new());
                    }
                    let fs = first_sec.unwrap();
                    self.sections[fs].flags |= HAS_SYMBOLS;
                    self.symtab.syms[si].sec = Some(fs);
                }
                // assign symbols to the section they are defined in.
                // Deliberate choice: vasm reads sec->idx of a *deleted* first
                // section here (an exported equate with an empty first section),
                // and new_section() never initialises idx. Codex observed the
                // 1.7h Mac build behaving as idx 0 (equate lands in the first
                // remaining hunk); we make that deterministic.
                let i = self.symtab.syms[si].sec.and_then(|s| idx.get(s).copied().flatten()).unwrap_or(0) as usize;
                secsyms[i].push(si);
            }
        }
        for l in secsyms.iter_mut() {
            l.reverse();
        }
        Prep { first_sec, sec_cnt, idx, secsyms }
    }

    /// file_size(): initialized data size of a section (for -databss)
    fn file_size(&mut self, sec: usize) -> u32 {
        let mut pc: Taddr = 0;
        let mut zpc: Taddr = 0;
        let n = self.sections[sec].atoms.len();
        for ai in 0..n {
            let mut a = std::mem::replace(&mut self.sections[sec].atoms[ai], Atom::rorgend());
            let npc = pcalign(&a, pc);
            let zerodata = match &a.kind {
                AtomKind::Data(db) => db.relocs.is_empty() && db.data.iter().all(|&b| b == 0),
                AtomKind::Space(sb) => sb.relocs.is_empty() && sb.fill[..sb.size].iter().all(|&b| b == 0),
                _ => true,
            };
            let size = self.atom_size(&mut a, sec, npc);
            self.sections[sec].atoms[ai] = a;
            pc = npc.wrapping_add(size as Taddr);
            if !zerodata {
                zpc = pc;
            }
        }
        zpc as u32
    }

    fn convert_reloc(&self, r: &Reloc, pc: u32, prep: &Prep) -> Option<HunkReloc> {
        let kick1 = self.opts.hunk_kick1;
        if r.kind <= LAST_STANDARD_RELOC {
            let s = &self.symtab.syms[r.sym];
            if s.is_locref() {
                let offs = pc.wrapping_add(r.byteoffset as u32);
                let ty = match r.kind {
                    REL_ABS => {
                        if r.size != 32 || r.bitoffset != 0 || r.mask != -1 {
                            return None;
                        }
                        HUNK_ABSRELOC32
                    }
                    REL_PC => match r.size {
                        8 => {
                            if r.bitoffset != 0 || r.mask != -1 {
                                return None;
                            }
                            HUNK_RELRELOC8
                        }
                        14 => {
                            if r.bitoffset != 0 || r.mask != 0xfffc {
                                return None;
                            }
                            HUNK_RELRELOC16
                        }
                        16 => {
                            if r.bitoffset != 0 || r.mask != -1 {
                                return None;
                            }
                            HUNK_RELRELOC16
                        }
                        32 => {
                            if kick1 || r.bitoffset != 0 || r.mask != -1 {
                                return None;
                            }
                            HUNK_RELRELOC32
                        }
                        _ => return None,
                    },
                    REL_SD => {
                        if kick1 || r.size != 16 || r.bitoffset != 0 || r.mask != -1 {
                            return None;
                        }
                        HUNK_DREL16
                    }
                    _ => return None,
                };
                let index = s.sec.and_then(|sec| prep.idx.get(sec).copied().flatten()).unwrap_or(0);
                return Some(HunkReloc { id: ty, index, offset: offs });
            }
        }
        None
    }

    fn convert_xref(&mut self, r: &Reloc, pc: u32) -> Option<HunkXref> {
        let kick1 = self.opts.hunk_kick1;
        if r.kind <= LAST_STANDARD_RELOC {
            let s = &self.symtab.syms[r.sym];
            if s.is_extref() {
                let offs = pc.wrapping_add(r.byteoffset as u32);
                let com = s.flags & COMMON != 0;
                let mut size = 0u32;
                let ty = match r.kind {
                    REL_ABS => {
                        if r.bitoffset != 0 || r.mask != -1 || (com && r.size != 32) {
                            return None;
                        }
                        match r.size {
                            8 => if kick1 { EXT_RELREF8 } else { EXT_ABSREF8 },
                            16 => if kick1 { EXT_RELREF16 } else { EXT_ABSREF16 },
                            32 => {
                                if com {
                                    size = self.get_sym_size(r.sym) as u32;
                                    EXT_ABSCOMMON
                                } else {
                                    EXT_ABSREF32
                                }
                            }
                            _ => return None,
                        }
                    }
                    REL_PC => match r.size {
                        8 => {
                            if r.bitoffset != 0 || r.mask != -1 || com {
                                return None;
                            }
                            EXT_RELREF8
                        }
                        14 => {
                            if r.bitoffset != 0 || r.mask != 0xfffc || com {
                                return None;
                            }
                            EXT_RELREF16
                        }
                        16 => {
                            if r.bitoffset != 0 || r.mask != -1 || com {
                                return None;
                            }
                            EXT_RELREF16
                        }
                        32 => {
                            if kick1 || r.bitoffset != 0 || r.mask != -1 {
                                return None;
                            }
                            if com {
                                size = self.get_sym_size(r.sym) as u32;
                                EXT_RELCOMMON
                            } else {
                                EXT_RELREF32
                            }
                        }
                        _ => return None,
                    },
                    REL_SD => {
                        if kick1 || r.size != 16 || r.bitoffset != 0 || r.mask != -1 {
                            return None;
                        }
                        EXT_DEXT16
                    }
                    _ => return None,
                };
                let name = self.symtab.syms[r.sym].name.clone();
                return Some(HunkXref { name, ty, size, offset: offs });
            }
        }
        None
    }

    /// unsupp_reloc_error() (reloc.c)
    fn unsupp_reloc_error(&mut self, r: &Reloc) {
        if r.kind <= LAST_STANDARD_RELOC {
            let name = self.symtab.syms[r.sym].name.clone();
            self.output_error(4, &[
                Arg::D(r.kind as i64),
                Arg::D(r.size as i64),
                Arg::S(format!("{:x}", r.mask as i64 as u64)),
                Arg::from(name),
                Arg::S(format!("{:x}", r.addend as i64 as u64)),
            ]);
        } else {
            self.output_error(5, &[Arg::D(r.kind as i64)]);
        }
    }

    /// process_relocs(): convert an atom's relocs into hunk relocs and xrefs.
    /// vasm prepends relocs to its list, hence the reverse iteration.
    fn process_relocs(&mut self, relocs: &[Reloc], reloclist: &mut Vec<HunkReloc>, mut xreflist: Option<&mut Vec<HunkXref>>, sec: usize, pc: u32, prep: &Prep) {
        for rl in relocs.iter().rev() {
            let hr = self.convert_reloc(rl, pc, prep);
            match hr {
                Some(hr) if xreflist.is_some() || hr.id == HUNK_ABSRELOC32 || hr.id == HUNK_RELRELOC32 => {
                    reloclist.push(hr);
                }
                _ => match self.convert_xref(rl, pc) {
                    Some(x) => match xreflist.as_deref_mut() {
                        Some(l) => l.push(x),
                        None => {
                            let secname = self.sections[sec].name.clone();
                            self.output_error(8, &[Arg::S(x.name), Arg::S(secname), Arg::S(format!("{:x}", x.offset)), Arg::D(rl.kind as i64)]);
                        }
                    },
                    None => self.unsupp_reloc_error(rl),
                },
            }
        }
    }

    /// reloc_hunk(): write all section offsets for one relocation type
    fn reloc_hunk(&self, out: &mut Vec<u8>, ty: u32, shrt: bool, reloclist: &mut Vec<HunkReloc>, sec_cnt: u32) {
        let mut bytes: u32 = 0;
        for idx in 0..sec_cnt {
            let mut n: u32 = 0;
            let mut off16 = true;
            for r in reloclist.iter() {
                if r.id == ty && r.index == idx {
                    n += 1;
                    if r.offset >= 0x10000 {
                        off16 = false;
                    }
                }
            }
            if shrt && (n >= 0x10000 || !off16) {
                continue; // relocs for this hunk don't fit into 16-bit entries
            }
            if n > 0 {
                if bytes == 0 {
                    if shrt && ty == HUNK_ABSRELOC32 {
                        fw32(out, HUNK_DREL32); // RELOC32SHORT is DREL32 for OS2.0
                    } else {
                        fw32(out, ty);
                    }
                    bytes = 4;
                }
                if shrt {
                    fw16(out, n as u16);
                    fw16(out, idx as u16);
                    bytes += 4;
                } else {
                    fw32(out, n);
                    fw32(out, idx);
                    bytes += 8;
                }
                reloclist.retain(|r| {
                    if r.id == ty && r.index == idx {
                        if shrt {
                            fw16(out, r.offset as u16);
                            bytes += 2;
                        } else {
                            fw32(out, r.offset);
                            bytes += 4;
                        }
                        false
                    } else {
                        true
                    }
                });
            }
        }
        if bytes != 0 {
            if shrt {
                fw16(out, 0);
                fwalign(out, bytes + 2, 4);
            } else {
                fw32(out, 0);
            }
        }
    }

    fn linedebug_hunk(&self, out: &mut Vec<u8>, list: &[HunkLine]) {
        if !list.is_empty() {
            let debugname = self.filename.clone().unwrap_or_default();
            let srcname_len = strlen32(&debugname);
            fw32(out, HUNK_DEBUG);
            fw32(out, srcname_len + (list.len() as u32) * 2 + 3);
            fw32(out, 0);
            fw32(out, 0x4c494e45); // "LINE"
            fw32(out, srcname_len);
            fwname(out, &debugname);
            for hl in list {
                fw32(out, hl.line);
                fw32(out, hl.offset);
            }
        }
    }

    fn extheader(out: &mut Vec<u8>, exthunk: &mut bool) {
        if !*exthunk {
            *exthunk = true;
            fw32(out, HUNK_EXT);
        }
    }

    /// ext_refs(): all external references from a section into a HUNK_EXT
    fn ext_refs(out: &mut Vec<u8>, xreflist: &mut Vec<HunkXref>, exthunk: &mut bool) {
        while !xreflist.is_empty() {
            Self::extheader(out, exthunk);
            let name = xreflist[0].name.clone();
            let ty = xreflist[0].ty;
            let size = xreflist[0].size;
            let n = xreflist.iter().filter(|x| x.name == name && x.ty == ty).count() as u32;
            fw32(out, (ty << 24) | strlen32(&name));
            fwname(out, &name);
            if ty == EXT_ABSCOMMON || ty == EXT_RELCOMMON {
                fw32(out, size);
            }
            fw32(out, n);
            xreflist.retain(|x| {
                if x.name == name && x.ty == ty {
                    fw32(out, x.offset);
                    false
                } else {
                    true
                }
            });
        }
    }

    fn ext_defs(&mut self, out: &mut Vec<u8>, symtype: SymKind, global: u32, idx: usize, xtype: u32, exthunk: &mut bool, prep: &Prep) {
        let mut header = false;
        for k in 0..prep.secsyms[idx].len() {
            let si = prep.secsyms[idx][k];
            let s = &self.symtab.syms[si];
            if s.kind == symtype && s.flags & global == global {
                if !header {
                    header = true;
                    if xtype == EXT_SYMB {
                        fw32(out, HUNK_SYMBOL);
                    } else {
                        Self::extheader(out, exthunk);
                    }
                }
                let name = s.name.clone();
                fw32(out, (xtype << 24) | strlen32(&name));
                fwname(out, &name);
                let v = self.get_sym_value(si);
                fw32(out, v as u32);
            }
        }
        if header && xtype == EXT_SYMB {
            fw32(out, 0);
        }
    }

    /// Write one section's contents (shared by object and executable output).
    /// Returns the final pc. `xreflist` is None for executables.
    fn write_contents(&mut self, out: &mut Vec<u8>, sec: usize, ty: u32, limit: Option<u32>, reloclist: &mut Vec<HunkReloc>, mut xreflist: Option<&mut Vec<HunkXref>>, linedb: &mut Vec<HunkLine>, prep: &Prep) -> u32 {
        let genlinedebug = self.opts.hunk_linedebug;
        let mut pc: u32 = 0;
        let n = self.sections[sec].atoms.len();
        for ai in 0..n {
            if let Some(l) = limit {
                if pc >= l {
                    break;
                }
            }
            let npc = self.fwpcalign(out, sec, ai, pc as u64) as u32;
            let mut a = std::mem::replace(&mut self.sections[sec].atoms[ai], Atom::rorgend());
            if genlinedebug && matches!(a.kind, AtomKind::Data(_) | AtomKind::Space(_)) {
                linedb.push(HunkLine { line: a.line as u32, offset: npc });
            }
            match &a.kind {
                AtomKind::Data(db) => {
                    out.extend_from_slice(&db.data);
                    self.process_relocs(&db.relocs, reloclist, xreflist.as_deref_mut(), sec, npc, prep);
                }
                AtomKind::Space(sb) => {
                    fwsblock(out, sb);
                    self.process_relocs(&sb.relocs, reloclist, xreflist.as_deref_mut(), sec, npc, prep);
                }
                AtomKind::Line(l) if !genlinedebug => {
                    linedb.push(HunkLine { line: *l as u32, offset: npc });
                }
                _ => {}
            }
            let size = self.atom_size(&mut a, sec, npc as Taddr);
            self.sections[sec].atoms[ai] = a;
            pc = npc.wrapping_add(size as u32);
        }
        if ty == HUNK_CODE && pc & 1 == 0 {
            self.fwnopalign(out, pc);
        } else {
            fwalign(out, pc, 4);
        }
        pc
    }

    /// write_object(): -Fhunk
    fn write_object(&mut self, out: &mut Vec<u8>) {
        let prep = self.prepare_sections();
        let mut wrotesec = false;
        let fname = self.filename.clone().unwrap_or_default();
        fw32(out, HUNK_UNIT);
        fw32(out, strlen32(&fname));
        fwname(out, &fname);

        let secs: Vec<usize> = (0..self.sections.len()).filter(|&s| prep.idx[s].is_some()).collect();
        for &sec in &secs {
            let idx = prep.idx[sec].unwrap();
            let mut reloclist: Vec<HunkReloc> = Vec::new();
            let mut xreflist: Vec<HunkXref> = Vec::new();
            let mut linedb: Vec<HunkLine> = Vec::new();
            wrotesec = true;

            // section name
            let name = self.sections[sec].name.clone();
            if !name.is_empty() {
                fw32(out, HUNK_NAME);
                fw32(out, strlen32(&name));
                fwname(out, &name);
            }
            // section type
            let attr = self.sections[sec].attr.clone();
            let mut ty = scan_attr(&attr);
            if ty == 0 {
                self.output_error(3, &[Arg::from(attr)]);
                ty = HUNK_DATA;
            }
            self.fwmemflags(out, sec, ty);
            fw32(out, (self.get_sec_size(sec).wrapping_add(3)) >> 2);

            if ty != HUNK_BSS {
                self.write_contents(out, sec, ty, None, &mut reloclist, Some(&mut xreflist), &mut linedb, &prep);
            }

            // relocation hunks
            for t in [HUNK_ABSRELOC32, HUNK_RELRELOC8, HUNK_RELRELOC16, HUNK_RELRELOC26, HUNK_RELRELOC32, HUNK_DREL16] {
                self.reloc_hunk(out, t, false, &mut reloclist, prep.sec_cnt);
            }

            // external references and global definitions
            let mut exthunk = false;
            Self::ext_refs(out, &mut xreflist, &mut exthunk);
            if idx == 0 {
                // absolute definitions into first hunk
                self.ext_defs(out, SymKind::Expression, EXPORT, 0, EXT_ABS, &mut exthunk, &prep);
            }
            self.ext_defs(out, SymKind::LabSym, EXPORT, idx as usize, EXT_DEF, &mut exthunk, &prep);
            if exthunk {
                fw32(out, 0);
            }

            if !self.opts.no_symbols {
                if !self.hunk_onlyglobal {
                    self.ext_defs(out, SymKind::LabSym, 0, idx as usize, EXT_SYMB, &mut exthunk, &prep);
                }
                self.linedebug_hunk(out, &linedb);
            }
            fw32(out, HUNK_END);
        }
        if !wrotesec {
            // there was no section at all - dummy section size 0
            fw32(out, HUNK_CODE);
            fw32(out, 0);
            fw32(out, HUNK_END);
        }
        let _ = prep.first_sec;
    }

    /// write_exec(): -Fhunkexe
    fn write_exec(&mut self, out: &mut Vec<u8>) {
        let prep = self.prepare_sections();
        let kick1 = self.opts.hunk_kick1;
        let databss = self.opts.hunk_databss;
        fw32(out, HUNK_HEADER);
        fw32(out, 0);
        if prep.sec_cnt != 0 {
            fw32(out, prep.sec_cnt); // number of sections - no overlay support
            fw32(out, 0); // first section index
            fw32(out, prep.sec_cnt - 1); // last section index
            let secs: Vec<usize> = (0..self.sections.len()).filter(|&s| prep.idx[s].is_some()).collect();
            // section sizes and memory flags
            for &sec in &secs {
                let sz = self.get_sec_size(sec).wrapping_add(3) >> 2;
                self.fwmemflags(out, sec, sz);
            }
            // section hunk loop
            for &sec in &secs {
                let idx = prep.idx[sec].unwrap();
                let mut reloclist: Vec<HunkReloc> = Vec::new();
                let mut linedb: Vec<HunkLine> = Vec::new();
                let attr = self.sections[sec].attr.clone();
                let mut ty = scan_attr(&attr);
                if ty == 0 {
                    self.output_error(3, &[Arg::from(attr)]);
                    ty = HUNK_DATA;
                }
                fw32(out, ty);
                if ty != HUNK_BSS {
                    let size = if databss { self.file_size(sec) } else { self.get_sec_size(sec) };
                    fw32(out, size.wrapping_add(3) >> 2);
                    self.write_contents(out, sec, ty, Some(size), &mut reloclist, None, &mut linedb, &prep);
                } else {
                    fw32(out, self.get_sec_size(sec).wrapping_add(3) >> 2);
                }
                if !kick1 {
                    self.reloc_hunk(out, HUNK_ABSRELOC32, true, &mut reloclist, prep.sec_cnt);
                }
                self.reloc_hunk(out, HUNK_ABSRELOC32, false, &mut reloclist, prep.sec_cnt);
                self.reloc_hunk(out, HUNK_RELRELOC32, true, &mut reloclist, prep.sec_cnt);

                if !self.opts.no_symbols {
                    let mut exthunk = false;
                    if !self.hunk_onlyglobal {
                        self.ext_defs(out, SymKind::LabSym, 0, idx as usize, EXT_SYMB, &mut exthunk, &prep);
                    }
                    self.linedebug_hunk(out, &linedb);
                }
                fw32(out, HUNK_END);
            }
        } else {
            // no sections: create single code hunk with size 0
            fw32(out, 1);
            fw32(out, 0);
            fw32(out, 0);
            fw32(out, 0);
            fw32(out, HUNK_CODE);
            fw32(out, 0);
            fw32(out, HUNK_END);
        }
    }

    /// write_output() for hunk (object) and hunkexe (executable)
    pub fn write_hunk(&mut self, exec: bool) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        if exec {
            self.write_exec(&mut out);
        } else {
            self.write_object(&mut out);
        }
        out
    }
}
