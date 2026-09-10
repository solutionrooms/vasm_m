//! -Fbin output module, transliterated from vasm 1.7h output_bin.c.
use crate::asm::Assembler;
use crate::atoms::*;
use crate::errors::Arg;
use crate::symbols::SymKind;
use std::io::Write;

impl Assembler {
    /// fwpcalign(): write alignment padding; returns the aligned pc
    fn fwpcalign(&mut self, out: &mut Vec<u8>, sec: usize, ai: usize, pc: u64) -> u64 {
        let a = &self.sections[sec].atoms[ai];
        let mut n = balign(pc as i32, a.align) as u32 as u64;
        if n == 0 {
            return pc;
        }
        let (pat, patlen): (Vec<u8>, usize) = match &a.kind {
            AtomKind::Space(sb) if sb.space == 0 => {
                if sb.maxalignbytes != 0 && n > sb.maxalignbytes as u64 {
                    return pc;
                }
                (sb.fill[..sb.size].to_vec(), sb.size)
            }
            _ => (self.sections[sec].pad.clone(), self.sections[sec].pad.len()),
        };
        let pc = pc + n;
        while n as usize % patlen != 0 {
            out.push(0);
            n -= 1;
        }
        while n as usize >= patlen {
            out.extend_from_slice(&pat);
            n -= patlen as u64;
        }
        while n > 0 {
            out.push(0);
            n -= 1;
        }
        pc
    }

    /// write_output() for the bin format
    pub fn write_bin(&mut self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        // undefined symbols
        for i in 0..self.symtab.syms.len() {
            if self.symtab.syms[i].kind == SymKind::Import {
                let n = self.symtab.syms[i].name.clone();
                self.output_error(6, &[Arg::from(n)]);
            }
        }
        // sections in output: all but UNALLOCATED (remove_unalloc_sects)
        let secs: Vec<usize> = (0..self.sections.len()).filter(|&s| self.sections[s].flags & UNALLOCATED == 0).collect();
        if secs.is_empty() {
            return out;
        }
        // overlap check
        for (k, &s) in secs.iter().enumerate() {
            for &s2 in &secs[k + 1..] {
                let (so, sp) = (self.sections[s].org as u32 as u64, self.sections[s].pc as u32 as u64);
                let (s2o, s2p) = (self.sections[s2].org as u32 as u64, self.sections[s2].pc as u32 as u64);
                if (s2o >= so && s2o < sp) || (s2p > so && s2p <= sp) {
                    self.output_error(0, &[]);
                }
            }
        }
        let mut sorted = secs.clone();
        if sorted.len() > 1 {
            sorted.sort_by_key(|&s| self.sections[s].org as u32);
        }
        let mut pc: u64 = 0;
        for (k, &s) in sorted.iter().enumerate() {
            let org = self.sections[s].org as u32 as u64;
            if k != 0 && org > pc {
                while pc < org {
                    out.push(0);
                    pc += 1;
                }
            } else {
                pc = org;
            }
            let n = self.sections[s].atoms.len();
            for ai in 0..n {
                let npc = self.fwpcalign(&mut out, s, ai, pc);
                match &self.sections[s].atoms[ai].kind {
                    AtomKind::Data(db) => out.extend_from_slice(&db.data),
                    AtomKind::Space(sb) => {
                        for _ in 0..sb.space {
                            out.extend_from_slice(&sb.fill[..sb.size]);
                        }
                    }
                    _ => {}
                }
                let mut a = std::mem::replace(&mut self.sections[s].atoms[ai], Atom::rorgend());
                let size = self.atom_size(&mut a, s, npc as i32);
                self.sections[s].atoms[ai] = a;
                pc = npc + size as u64;
            }
        }
        out
    }

    pub fn write_output_file(&mut self, data: &[u8]) {
        let name = self.opts.output.clone().unwrap_or_else(|| "a.out".to_string());
        match std::fs::File::create(&name) {
            Ok(mut f) => {
                if f.write_all(data).is_err() {
                    self.output_error(2, &[]);
                }
            }
            Err(_) => {
                self.general_error(13, &[Arg::from(name)]);
            }
        }
    }
}
