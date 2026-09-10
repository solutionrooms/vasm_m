<!-- Generated 2026-09-10 by a research agent reading vasm-1.7h source; line numbers refer to that tree. -->

# vasm 1.7h — `vasmm68k_mot -Fbin` reimplementation spec

Paths are relative to `/Users/jonscott/Projects/vasm_m/vasm-1.7h/`. All line numbers are from the files as they exist in that tree.

Key type facts (`cpus/m68k/cpu.h:6-21`, `cpus/m68k/cpu.c:29-30`):
```c
#define BIGENDIAN 1
#define LITTLEENDIAN 0
#define MAX_OPERANDS 6
#define MAX_QUALIFIERS 1
typedef int32_t  taddr;
typedef uint32_t utaddr;
int bitsperbyte = 8;
int bytespertaddr = 4;
#define INST_ALIGN 2
#define DATA_ALIGN(n) ((n<=8)?1:2)
```
`taddrmask = MAKEMASK(bytespertaddr<<3)` = `(1LL<<32)-1` (`vasm.c:528`, `reloc.h:48`).

**`BOOLEAN(x)` is `-(x)` in the mot syntax module** (`syntax/mot/syntax.h:18`). Every comparison/logical operator therefore yields **-1** for true, 0 for false — not 1. This is a frequent source of byte differences.

---

## 1. Main driver: pass structure (`vasm.c`)

### 1.1 Constants

`vasm.c:24-25`, with the authoritative comment at `vasm.c:18-23`:
```c
/* The resolver will run another pass over the current section as long as any
   label location or atom size has changed. It gives up at MAXPASSES, which
   hopefully will never happen.
   During the first FASTOPTPHASE passes all instructions of a section will be
   optimized at the same time. After that the resolver enters a safe mode,
   where only a single instruction per pass is changed. */
#define MAXPASSES 1000
#define FASTOPTPHASE 50
```

`MAXSIZECHANGES 5` (`atom.h:110`) — "warning, when atom changed size so many times".

There is **no** `unresolved` and no `nopass` in vasm 1.7h. The equivalents are the file-scope `int done` (`vasm.c:37`), the per-atom `unsigned changes` counter (`atom.h:89`), and the section flag `RESOLVE_WARN` (`vasm.h:105`). If you saw those names elsewhere they're from a different assembler.

### 1.2 `resolve_section()` — quoted in full (`vasm.c:174-268`)

```c
static void resolve_section(section *sec)
{
  taddr rorg_pc,org_pc;
  int fastphase=FASTOPTPHASE;
  int pass=0;
  int extrapass;
  size_t size;
  atom *p;

  do{
    done=1;
    rorg_pc=0;
    if (++pass>=MAXPASSES){
      general_error(7,sec->name);
      break;
    }
    extrapass=pass<=fastphase;
    if(debug)
      printf("resolve_section(%s) pass %d%s",sec->name,pass,
             pass<=fastphase?" (fast)\n":"\n");
    sec->pc=sec->org;
    for(p=sec->first;p;p=p->next){
      sec->pc=pcalign(p,sec->pc);
      cur_src=p->src;
      cur_src->line=p->line;
#if HAVE_CPU_OPTS
      if(p->type==OPTS){
        cpu_opts(p->content.opts);
      }
      else
#endif
      if(p->type==RORG){
        if(rorg_pc!=0)
          general_error(43);  /* reloc org is already set */
        rorg_pc=*p->content.rorg;
        org_pc=sec->pc;
        sec->pc=rorg_pc;
        sec->flags|=ABSOLUTE;
      }
      else if(p->type==RORGEND&&rorg_pc!=0){
        sec->pc=org_pc+(sec->pc-rorg_pc);
        rorg_pc=0;
        sec->flags&=~ABSOLUTE;
      }
      else if(p->type==LABEL){
        symbol *label=p->content.label;
        if(label->type!=LABSYM)
          ierror(0);
        if(label->pc!=sec->pc){
          if(debug)
            printf("moving label %s from %lu to %lu\n",label->name,
                   (unsigned long)label->pc,(unsigned long)sec->pc);
          done=0;
          label->pc=sec->pc;
        }
      }
      if(pass>fastphase&&!done&&p->type==INSTRUCTION){
        /* entered safe mode: optimize only one instruction every pass */
        sec->pc+=p->lastsize;
        continue;
      }
      if(p->changes>MAXSIZECHANGES){
        /* atom changed size too frequently, set warning flag */
        if(debug)
          printf("setting resolve-warning flag for atom type %d at %lu\n",
                 p->type,(unsigned long)sec->pc);
        sec->flags|=RESOLVE_WARN;
        size=atom_size(p,sec,sec->pc);
        sec->flags&=~RESOLVE_WARN;
      }
      else
        size=atom_size(p,sec,sec->pc);
      if(size!=p->lastsize){
        if(debug)
          printf("modify size of atom type %d at %lu from %lu to %lu\n",
                 p->type,(unsigned long)sec->pc,(unsigned long)p->lastsize,
                 (unsigned long)size);
        done=0;
        if(pass>fastphase)
          p->changes++;  /* now count size modifications of atoms */
        else if(size>p->lastsize)
          extrapass=0;   /* no extra pass, when an atom became larger */
        p->lastsize=size;
      }
      sec->pc+=size;
    }
    if(rorg_pc!=0){
      sec->pc=org_pc+(sec->pc-rorg_pc);
      sec->flags&=~ABSOLUTE;  /* workaround for misssing RORGEND */
    }
    /* Extend the fast-optimization phase, when there was no atom which
       became larger than in the previous pass. */
    if(extrapass) fastphase++;
  }while(errors==0&&!done);
}
```

Semantics to replicate exactly:

- `pass` starts at 0 and is pre-incremented at the top, so the first iteration runs with `pass == 1`. Bail-out is `++pass >= MAXPASSES`, i.e. the loop body never executes with `pass == 1000`; passes 1..999 execute.
- `fastphase` is a **local mutable** initialised to `FASTOPTPHASE` (50). It is incremented at the end of any pass where `extrapass` stayed true — so the "fast" phase is extended indefinitely as long as no atom grew during that pass.
- `extrapass` is set at the start of every pass to `pass<=fastphase`, and cleared only in the fast phase when an atom **grew** (`size>p->lastsize`). Shrinking atoms do not clear it.
- **Safe mode** (`pass>fastphase`): once `done` has already been cleared during this pass, every subsequent `INSTRUCTION` atom is skipped — `sec->pc += p->lastsize; continue;`. Its size is not recomputed. That is how "only a single instruction per pass is changed" is implemented: the first instruction in the pass that changes size clears `done`, and every later instruction in that same pass is frozen at its previous size. Note the check is `!done`, so a *label* move earlier in the pass also freezes all later instructions in that pass.
- In safe mode, size changes increment `p->changes`. Once `p->changes > MAXSIZECHANGES` (i.e. ≥ 6), the section flag `RESOLVE_WARN` is set around the `atom_size()` call so the CPU backend can emit a warning and stop oscillating.
- `pcalign(p,sec->pc)` is applied before each atom (`supp.c`):
  ```c
  taddr balign(taddr addr,taddr a) { return a ? (((addr+a-1)&~(a-1)) - addr) : 0; }
  taddr pcalign(atom *a,taddr pc)
  {
    taddr n = balign(pc,a->align);
    if (a->type==SPACE && a->content.sb->maxalignbytes!=0)
      if (n > a->content.sb->maxalignbytes) n = 0;
    return pc + n;
  }
  ```
  `maxalignbytes` is never set by the mot module (`new_sblock` sets it to 0, `atom.c:150`), so the second clause is dead for this configuration.
- Label PC assignment: labels are `LABEL` atoms of size 0 and `align 1`. Their `pc` is written to `sec->pc` *after* `pcalign` for that atom. Because `add_atom()` propagates alignment (`atom.c:220-228`), a label immediately preceding an `INSTRUCTION`/`DATADEF`/`SPACE` atom **on the same source line** inherits that atom's alignment, so the label lands after the padding, not before it:
  ```c
  if (pa->type==LABEL && pa->line==a->line &&
      (a->type==INSTRUCTION || a->type==DATADEF || a->type==SPACE))
    pa->align = a->align;
  ```
- Termination: `while(errors==0 && !done)`. Any error aborts the loop for that section.
- Exceeding `MAXPASSES` → `general_error(7,sec->name)` = *"cannot resolve section <%s>, maximum number of passes reached"*, flags `NOLINE|ERROR|FATAL` (`general_errors.h:8`, printed as `error 8`). Because it's FATAL, `error()` calls `leave()` and the process exits (`error.c:155-158`). This is the "not converging" case — vasm has no separate "instruction not converging" message; the m68k backend has its own oscillation warnings triggered by `RESOLVE_WARN`.

### 1.3 `resolve()` and `assemble()`

`vasm.c:270-278`: `resolve()` sets `final_pass=0` and calls `resolve_section()` for each section **in declaration order** (`first_section` list).

`assemble()` (`vasm.c:280-423`) is the single final pass:
1. `convert_offset_labels()` (`vasm.c:115-126`) — every `LABSYM` in an `UNALLOCATED` (offset/`rs`-style) section becomes an `EXPRESSION` symbol with a constant value and `sec=NULL`.
2. `final_pass = 1`.
3. For each section, `sec->pc = sec->org`, then for each atom: apply `pcalign`; warn 50 (`instruction has been auto-aligned`) or 57 (`data has been auto-aligned`) if padding was inserted; handle `RORG`/`RORGEND`; convert `INSTRUCTION` → `DATA` via `eval_instruction()`; convert `DATADEF` → `DATA` via `eval_data()`; convert `ROFFS` → `SPACE`; execute `PRINTTEXT`/`PRINTEXPR`/`ASSERT`/`NLIST`; error 31 if `DATA` lands in a `u`-attributed (bss) section; `sec->pc += atom_size(...)`.
4. `remove_unalloc_sects()` (`vasm.c:129-143`) drops `UNALLOCATED` sections from the output list.

After `assemble()`: `undef_syms()` (only if `errors==0`), then `fix_labels()` (`vasm.c:437-466`) which turns `ABSLABEL` LABSYMs into constant EXPRESSIONs and re-bases relocatable EXPRESSION symbols. Then listing, then output (`vasm.c:769-797`).

`main()` order (`vasm.c:761-797`):
```
set_input_name(); internal_abs("__VASM"); init_parse(); init_syntax(); init_cpu();
parse();
if(errors==0||produce_listing) resolve();
if(errors==0||produce_listing) assemble();
undef_syms(); fix_labels();
if(errors==0){ statistics(); outfile=fopen(outname?:"a.out","wb"); write_object(...); }
leave();
```
`leave()` (`vasm.c:85-112`) closes and **`remove()`s the output file if `errors != 0`**.

---

## 2. Atom types (`atom.h`, `atom.c`)

`atom.h:8-21` — note there is **no `LIST` atom type** in 1.7h; the list is:
```c
#define LABEL 1
#define DATA  2
#define INSTRUCTION 3
#define SPACE 4
#define DATADEF 5
#define LINE 6
#define OPTS 7
#define PRINTTEXT 8
#define PRINTEXPR 9
#define ROFFS 10
#define RORG 11
#define RORGEND 12
#define ASSERT 13
#define NLIST 14
```

Atom struct (`atom.h:84-108`):
```c
typedef struct atom {
  struct atom *next;
  int type;
  taddr align;
  size_t lastsize;
  unsigned changes;
  source *src;
  int line;
  listing *list;
  union { instruction *inst; dblock *db; symbol *label; sblock *sb;
          defblock *defb; void *opts; int srcline; char *ptext;
          printexpr *pexpr; expr *roffs; taddr *rorg;
          assertion *assert; aoutnlist *nlist; } content;
} atom;
```

### 2.1 `atom_size()` (`atom.c:252-281`) — verbatim table

| type | size | notes |
|---|---|---|
| `LABEL`, `LINE`, `OPTS`, `PRINTTEXT`, `PRINTEXPR`, `RORG`, `RORGEND`, `ASSERT`, `NLIST` | 0 | |
| `DATA` | `p->content.db->size` | |
| `INSTRUCTION` | `code>=0 ? instruction_size(inst,sec,pc) : 0` | CPU module |
| `SPACE` | `space_size(sb,sec,pc)` | |
| `DATADEF` | `(bitsize+7)/8` | |
| `ROFFS` | `roffs_size(roffs,sec,pc)` | |

Default alignments assigned by the constructors (`atom.c:449-601`):
- `new_inst_atom` → `INST_ALIGN` = **2**
- `new_label_atom`, `new_space_atom`, `new_srcline_atom`, `new_opts_atom`, `new_text_atom`, `new_expr_atom`, `new_roffs_atom`, `new_rorg_atom`, `new_rorgend_atom`, `new_assert_atom`, `new_nlist_atom` → **1**
- `new_datadef_atom(bitsize,op)` → `DATA_ALIGN(bitsize)` = `(bitsize<=8)?1:2`
- `new_data_atom(db,align)` → caller-supplied

**Crucially, the mot syntax module overrides data alignment back to 1 unless `-align` is given** (`syntax.c:608-609` and `syntax.c:292`):
```c
a = new_datadef_atom(OPSZ_BITS(size),op);
if (!align_data) a->align = 1;
...
a = new_space_atom(cnt,size>>3,fill);
a->align = align_data ? DATA_ALIGN(size) : 1;
```
So by default `dc.w`/`dc.l`/`ds.w` are **not** auto-aligned; only instructions (`INST_ALIGN 2`) and explicit `cnop`/`even`/`align` produce padding. There is no separate `cpu_align` field.

`add_atom()` also bumps the section alignment: `if (a->align > sec->align) sec->align = a->align;` (`atom.c:237-238`). For `-Fbin` this only matters for the `statistics()` printout.

### 2.2 `SPACE` atoms with a non-constant size — `space_size()` (`atom.c:155-191`)

```c
static size_t space_size(sblock *sb,section *sec,taddr pc)
{
  utaddr space=0;

  if (eval_expr(sb->space_exp,&space,sec,pc) || !final_pass) {
    sb->space = space;
    if ((utaddr)(pc+space) < (utaddr)pc)
      general_error(45);  /* illegal negative value */
  }
  else
    general_error(30);  /* expression must be constant */

  if (final_pass && sb->fill_exp) {
    if (sb->size <= sizeof(taddr)) {
      symbol *base=NULL;
      taddr fill;
      utaddr i;

      if (!eval_expr(sb->fill_exp,&fill,sec,pc)) {
        if (find_base(sb->fill_exp,&base,sec,pc)==BASE_ILLEGAL)
          general_error(38);  /* illegal relocation */
      }
      copy_cpu_taddr(sb->fill,fill,sb->size);
      if (base && !sb->relocs) {
        for (i=0; i<space; i++)
          add_extnreloc(&sb->relocs,base,fill,REL_ABS,0,sb->size<<3,sb->size*i);
      }
    }
    else
      general_error(30);
  }
  return sb->size * space;
}
```

Rules:
- During resolve passes (`final_pass==0`), a non-constant count is **accepted silently** and the current (partial) value is stored in `sb->space`. That is how `ds.b end-start` converges.
- In the final pass, a non-constant count raises error 30 and **`sb->space` is left at its last resolve-pass value** (the assignment is inside the `if`). Your implementation must not zero it.
- Negative/overflowing sizes → error 45 (`illegal negative value`), tested as `(utaddr)(pc+space) < (utaddr)pc`.
- Fill: `copy_cpu_taddr` writes `sb->size` low-order bytes **big-endian** for m68k. `sb->fill` is `uint8_t fill[MAXPADBYTES]` with `MAXPADBYTES 8` (`vasm.h:21`, `atom.h:52`). `sb->size > 4` with an explicit fill → error 30. `ds.x` / `dcb.x` use `size=12 > MAXPADBYTES` and `fwsblock` then writes 12 bytes from an 8-byte array — a genuine over-read in vasm; on normal builds the extra 4 bytes are the zeroed low half of `fill_exp`, so it emits 12 zeros. Emit 12 zeros.

`roffs_size()` (`atom.c:194-201`):
```c
eval_expr(offsexp,&offs,sec,pc);
offs = sec->org + offs - pc;
return offs>0 ? offs : 0;
```

`add_atom()` (`atom.c:206-249`) sets `a->changes=0`, `a->src=cur_src`, `a->line=cur_src->line`, links the atom, propagates label alignment as shown above, then does `sec->pc = pcalign(a,sec->pc); a->lastsize = atom_size(a,sec,sec->pc); sec->pc += a->lastsize;`.

---

## 3. Sections and `-Fbin` output

### 3.1 `struct section` (`vasm.h:114-128`)

```c
struct section {
  struct section *next;
  char *name;
  char *attr;
  atom *first;
  atom *last;
  taddr align;
  uint8_t pad[MAXPADBYTES];
  int padbytes;
  uint32_t flags;
  uint32_t memattr;
  taddr org;
  taddr pc;
  uint32_t idx;
};
```
Flags (`vasm.h:104-111`): `HAS_SYMBOLS 1`, `RESOLVE_WARN 2`, `UNALLOCATED 4`, `LABELS_ARE_LOCAL 8`, `ABSOLUTE 16`, `PREVABS 32`, `IN_RORG 64`.

`new_section()` (`vasm.c:1016-1039`): looks up by name (and attr, since `secname_attr=1` in mot — `syntax.c:2224`), else allocates with `align` from the argument (mot always passes 1), `org=pc=0`, `flags=0`, `memattr=0`, **`pad` all zero and `padbytes=1`**. The mot module's NOP-padding block is `#if NOT_NEEDED` (`syntax.c:386-393`), so `sec->pad` is always a single 0x00.

Default section (`vasm.c:1090-1099`): if no section is current and an atom is added, `default_section()` creates and switches to `defsectname`/`defsecttype` = `"CODE"` / `"acrx"` (`syntax.c:19,22,29-30`).

`new_org()` (`vasm.c:1042-1052`):
```c
sprintf(buf,"seg%llx",ULLTADDR(org));
sec = new_section(buf,"acrwx",1);
sec->org = sec->pc = org;
sec->flags |= ABSOLUTE;
```
Because `new_section` de-duplicates by name+attr, two `org $1000` directives return the **same** section and continue appending to it.

`handle_org` (`syntax.c:500-515`) chooses between two very different behaviours:
```c
if (current_section!=NULL && !(current_section->flags & ABSOLUTE))
  start_rorg(parse_constexpr(&s));
else
  set_section(new_org(parse_constexpr(&s)));
```
So `org` at the very start of a file (no section yet) creates an absolute `segXXXX` section; `org` inside an already-established relocatable section starts a **RORG block** inside it (`vasm.c:1151-1169`) which only relocates *symbol values*, not file layout.

`switch_offset_section()` (`vasm.c:1069-1086`) creates `OFFSET%06lu` sections with attr `"u"` and `UNALLOCATED`; these are removed before output.

### 3.2 `-Fbin` (`output_bin.c`)

```c
static void write_output(FILE *f,section *sec,symbol *sym)
{
  section *s,*s2,**seclist,**slp;
  atom *p;
  size_t nsecs;
  unsigned long long pc,npc,i;

  if (!sec) return;

  for (; sym; sym=sym->next) {
    if (sym->type == IMPORT)
      output_error(6,sym->name);  /* undefined symbol */
  }

  /* we don't support overlapping sections */
  for (s=sec,nsecs=0; s!=NULL; s=s->next) {
    for (s2=s->next; s2; s2=s2->next) {
      if (((ULLTADDR(s2->org) >= ULLTADDR(s->org) &&
            ULLTADDR(s2->org) <  ULLTADDR(s->pc)) ||
           (ULLTADDR(s2->pc)  >  ULLTADDR(s->org) &&
            ULLTADDR(s2->pc)  <= ULLTADDR(s->pc))))
        output_error(0);
    }
    nsecs++;
  }

  seclist = (section **)mymalloc(nsecs * sizeof(section *));
  for (s=sec,slp=seclist; s!=NULL; s=s->next) *slp++ = s;
  if (nsecs > 1) qsort(seclist,nsecs,sizeof(section *),orgcmp);

  if (binfmt == BINFMT_CBMPRG) { fw8(f,sec->org&0xff); fw8(f,(sec->org>>8)&0xff); }

  for (slp=seclist; nsecs>0; nsecs--) {
    s = *slp++;
    if (s!=seclist[0] && ULLTADDR(s->org)>pc) {
      for (; pc<ULLTADDR(s->org); pc++) fw8(f,0);   /* fill gap with zeros */
    }
    else
      pc = ULLTADDR(s->org);

    for (p=s->first; p; p=p->next) {
      npc = fwpcalign(f,p,s,pc);
      if (p->type == DATA) {
        for (i=0; i<p->content.db->size; i++)
          fw8(f,(unsigned char)p->content.db->data[i]);
      }
      else if (p->type == SPACE) {
        fwsblock(f,p->content.sb);
      }
      pc = npc + atom_size(p,s,npc);
    }
  }
  free(seclist);
}
```
(`output_bin.c:24-90`; `orgcmp` at `:14-21`.)

Byte-exactness consequences:

1. **Ordering** — sections are sorted by `org` ascending (stable only insofar as `qsort` is; equal orgs keep unspecified relative order, but equal non-empty orgs already trip the overlap error).
2. **Undefined symbols** — any remaining `IMPORT` symbol → `output_error(6)` = error 3007 → with the default `max_errors=5`, five such errors abort; in all cases `leave()` deletes the output file because `errors != 0`.
3. **Overlap** — `output_error(0)` = error 3001 *"sections must not overlap"*. Note all ordinary relocatable sections have `org == 0`, so **two non-empty relocatable sections always overlap**. Practical `-Fbin` input therefore uses either exactly one section or `org`-created absolute sections.
4. **Gaps between sections are filled with 0x00**, one byte at a time. There is no leading pad: the file starts at the lowest `org`; addresses below it are simply not represented.
5. **The `else pc = ULLTADDR(s->org)` branch fires for the first section and for any later section whose `org <= pc`**, resetting `pc` (in the overlap case this silently rewinds the logical PC while the file position does not rewind — but that case has already errored).
6. **Trailing `ds`/`dcb` is written, not truncated.** `SPACE` atoms go through `fwsblock` (`supp.c:422-430`):
   ```c
   void fwsblock(FILE *f,sblock *sb)
   { size_t i; for (i=0; i<sb->space; i++) { if (!fwrite(sb->fill,sb->size,1,f)) output_error(2); } }
   ```
   A BSS-attributed section is *not* special-cased by the bin module: its `ds` bytes are emitted too. There is no file truncation anywhere.
7. **Alignment padding** — `fwpcalign` (`supp.c:450-500`):
   ```c
   taddr fwpcalign(FILE *f,atom *a,section *sec,taddr pc)
   {
     int align_warning = 0;
     taddr n = balign(pc,a->align);
     taddr patlen; uint8_t *pat;

     if (n == 0) return pc;

     if (a->type==SPACE && a->content.sb->space==0) {  /* space align atom */
       if (a->content.sb->maxalignbytes!=0 && n>a->content.sb->maxalignbytes) return pc;
       pat = a->content.sb->fill;
       patlen = a->content.sb->size;
     }
     else { pat = sec->pad; patlen = sec->padbytes; }

     pc += n;
     while (n % patlen) { ... fw8(f,0); n--; }
     while (n >= patlen) { if (!fwrite(pat,patlen,1,f)) output_error(2); n -= patlen; }
     while (n--) { ... fw8(f,0); }
     return pc;
   }
   ```
   This is the **NOP-padding mechanism**: a `cnop 0,align` in an m68k code section produces a `SPACE` atom whose `space == 0`, `size == 2`, `fill == {0x4E,0x71}`, so the padding is written as `4E71` words. Any other atom pads with `sec->pad` = a single `0x00`. Note the odd-leftover handling: `n % patlen` zero bytes are written **first**, then whole patterns, so an odd padding count in a NOP-padded cnop emits one leading `0x00` then `4E71`s.
8. **`RORG`/`RORGEND` atoms are ignored by the bin writer.** They have size 0 and no case in the loop, so a relocatable `org` block produces *no* gap in the file — it only shifts symbol values. Furthermore `fwpcalign` computes alignment against the section-relative `pc`, whereas `resolve_section`/`assemble` computed sizes against the *relocated* `rorg_pc`. If you use `org` inside a `section`, alignment can therefore differ between the size pass and the write pass. Replicate the code, not the intent.
9. **`-cbm-prg`** prepends a 2-byte little-endian load address taken from `sec->org` — the *first section in declaration order*, not the lowest-org one. Irrelevant for m68k but present.

Output file name defaults to `"a.out"` (`vasm.c:787-788`), opened `"wb"`.

---

## 4. Expression evaluator (`expr.c`)

### 4.1 Node types (`expr.h:6-22`)

```c
enum { ADD,SUB,MUL,DIV,MOD,NEG,CPL,LAND,LOR,BAND,BOR,XOR,NOT,LSH,RSH,RSHU,
       LT,GT,LEQ,GEQ,NEQ,EQ,NUM,HUG,FLT,SYM };
struct expr { int type; struct expr *left,*right;
              union { taddr val; tfloat flt; thuge huge; symbol *sym; } c; };
```

### 4.2 Grammar / precedence — **not C-like**

The recursive-descent chain, from tightest to loosest (`expr.c:79-516`):

| level | function | operators | line |
|---|---|---|---|
| 0 | `primary_expr` | `( )`, local label, number, `*` (current PC), identifier, `'…'`/`"…"` | 79 |
| 1 | `unary_expr` | prefix `+ - ! ~` | 268 |
| 2 | `shift_expr` | `<<` `>>` | 298 |
| 3 | `and_expr` | `&` (not `&&`) | 321 |
| 4 | `exclusive_or_expr` | `^` `~` | 339 |
| 5 | `inclusive_or_expr` | `\|` (not `\|\|`), `!` (not `!=`) | 357 |
| 6 | `multiplicative_expr` | `*` `/` `//` `%` | 375 |
| 7 | `additive_expr` | `+` `-` | 405 |
| 8 | `relational_expr` | `<` `>` `<=` `>=` | 427 |
| 9 | `equality_expr` | `=` `==` `!=` `<>` | 458 |
| 10 | `logical_and_expr` | `&&` | 482 |
| 11 | `expression` | `\|\|` | 500 |

So **shifts bind tighter than `&`, which binds tighter than `^`, which binds tighter than `|`, which binds tighter than `*`/`/`, which binds tighter than `+`/`-`**. `1+2*3` = 9, not 7. This matches `doc/vasm_main.texi:167-177`. All levels are left-associative.

Disambiguation details worth copying literally:
- `shift_expr`: `while((*s=='<'||*s=='>')&&s[1]==*s)` — requires the doubled character.
- `and_expr`: `while(*s=='&'&&s[1]!='&')`.
- `exclusive_or_expr`: accepts both `^` and `~` as binary XOR.
- `inclusive_or_expr`: `while((*s=='|'&&s[1]!='|')||(*s=='!'&&s[1]!='='))` — a lone `!` is binary OR.
- `multiplicative_expr`: `/` followed by another `/` is `MOD`; `%` is `MOD`.
- `additive_expr`: `while((*s=='+'&&s[1]!='+')||(*s=='-'&&s[1]!='-'))` — `++`/`--` terminate the expression.
- `relational_expr`: `while(((*s=='<'&&s[1]!='>')||*s=='>')&&s[1]!=*s)`.
- `equality_expr`: `while(*s=='='||(*s=='!'&&s[1]=='=')||(*s=='<'&&s[1]=='>'))`, and `if(m==*s||m!='=') s++;` handles both `=` and `==`.

`EXPSKIP()` is redefined by the mot module to `s = exp_skip(s)` (`syntax.h:21-22`), so whitespace handling inside expressions is governed by `-spaces` (see §6.2).

### 4.3 Numbers (`syntax.c:2140-2166`, `expr.c:110-189`)

```c
char *const_prefix(char *s,int *base)
{
  if (isdigit((unsigned char)*s))              { *base = 10; return s;   }
  if (*s == '$')                               { *base = 16; return s+1; }
  if (*s=='@' && isdigit((unsigned char)*(s+1))){ *base = 8;  return s+1; }
  if (*s == '%')                               { *base = 2;  return s+1; }
  *base = 0; return s;
}
char *const_suffix(char *start,char *end) { return end; }
```
No suffixes. `@` is octal only when followed by a digit (otherwise `@` is an identifier start char, `syntax.h:8`).

Parsing (`expr.c:116-188`): accumulate into `utaddr val`. For `base<=10`: `while(*s>='0' && *s < base+'0')`. Overflow test `nval/base != val` → `goto hugeval`. For base 16: `nval = val<<4`, overflow test `nval>>4 != val`. For base 10 only, an `e`/`E` or a `.` followed by a digit switches to float (`goto fltval`, `strtotfloat`). `HUG` mode re-parses from `m` into a 128-bit `thuge` with `haddi(hmuli(...))`.

Huge → taddr conversion at eval time (`expr.c:1055-1059`):
```c
case HUG:
  if (!huge_chkrange(tree->c.huge,bytespertaddr*8))
    general_error(21,bytespertaddr*8);  /* target data type overflow */
  val=huge_to_int(tree->c.huge);
```
`huge_to_int` is `return (int64_t)h.lo;` — a plain truncation to the low 64 bits, then implicitly to `taddr` (int32). Float → taddr uses `flt_chkrange` then `(taddr)tree->c.flt`.

### 4.4 `*` — current PC (`expr.c:190-200`)

```c
if(*s==current_pc_char && !ISIDCHAR(*(s+1))){
  s++;
  EXPSKIP();
  if(make_tmp_lab){
    new=new_expr(); new->type=SYM;
    new->c.sym=new_tmplabel(0);
    add_atom(0,new_label_atom(new->c.sym));
  }else new=curpc_expr();
  return new;
}
```
`current_pc_char = '*'` for mot (`syntax.c:2223`). `make_tmp_lab` is set by `parse_expr_tmplab()` and clear for `parse_expr()`. With `make_tmp_lab`, a real temporary label atom named `" *tmp%09lu*"` (`symbol.c:349-356`) is inserted into the current section at that point; without it, a shared internal `" *current pc dummy*"` LABSYM is used whose `sec`/`pc` are patched by `update_curpc()` on each evaluation (`expr.c:54-77`).

Which entry point the mot module uses matters (`syntax.c`):
- `parse_expr_tmplab`: `equ`, `=`, `set`, `ds.*`, `dcb.*`/`blk.*` (count and fill), `cnop` offset, `rorg`, `rs/so/fo`, `cargs` offset, all `if<cc>` expressions, `comment HEAD=`.
- `parse_expr` (no tmplab): `printv`, `-D` on the command line, and — via the CPU module — **all instruction and `dc.*` operands** (`cpus/m68k/cpu.c:1179,1188,1441`).
- `parse_expr_float`: `fequ.s/d/x/p`, and `dc.s/d/x` operands (`cpu.c:1175`).
- `parse_expr_huge`: `dc.q` operands (`cpu.c:1177`).
- `parse_constexpr`: section memory type, `offset`, `org`, `comm` size, `cnop` alignment, `align`, `plen`, `debug`, `rept` count, `rsset`/`setfo`. `parse_constexpr` (`expr.c:1422-1446`) simplifies and errors 59/60/30 for HUG/FLT/non-constant, returning 0 on failure.

### 4.5 Character/string constants in expressions (`expr.c:227-260`)

```c
if(*s=='\''||*s=='\"'){
  taddr val=0; int shift=0,cnt=0; char quote=*s++;
  while(*s){
    char c;
    if(*s=='\\') s=escape(s,&c);
    else { c=*s++; if(c==quote){ if(*s==quote) s++; else break; } }
    if(++cnt>bytespertaddr){ general_error(21,bytespertaddr*8); break; }
    if(BIGENDIAN){ val=(val<<8)+c; }
    else if(LITTLEENDIAN){ val+=c<<shift; shift+=8; }
    else ierror(0);
  }
  ...
}
```
Both quote characters are accepted. Doubling the quote (`''''`, `""""`) yields one quote character. Max 4 characters for m68k, otherwise warning 21. Big-endian packing: `'AB'` → 0x4142.

`escape()` (`parse.c:32-102`) — **`esc_sequences` defaults to 0** (`parse.c:6`). When off:
```c
if (!esc_sequences) { *code='\\'; return s; }
```
i.e. a backslash produces a literal backslash and the following character is then processed normally. `"a\nb"` is 4 bytes `61 5C 6E 62`. Only `-esc` (or `-phxass`) enables `\b \f \n \r \t \\ \" \' \e \<octal, max 3 digits> \x<hex>`.

### 4.6 `eval_expr()` return convention (`expr.c:927-1074`)

Signature `int eval_expr(expr *tree, taddr *result, section *sec, taddr pc)`. Return 1 = constant, 0 = not constant. **`*result` is always written**, even when the return is 0. Leaves:

```c
case SYM:
  if(tree->c.sym->type==EXPRESSION){
    if(tree->c.sym->flags&INEVAL) general_error(18,tree->c.sym->name);
    tree->c.sym->flags|=INEVAL;
    cnst=eval_expr(tree->c.sym->expr,&val,sec,pc);
    tree->c.sym->flags&=~INEVAL;
  }else if(LOCREF(tree->c.sym)){
    update_curpc(tree,sec,pc);
    val=tree->c.sym->pc;
    cnst=tree->c.sym->sec==NULL?0:(tree->c.sym->sec->flags&UNALLOCATED)!=0;
  }else{
    /* IMPORT */
    cnst=0;
    val=0;
  }
  break;
```
So a normal label yields `cnst=0` but `val = sym->pc` (its *current* address). That is precisely why the multi-pass resolver converges: instruction sizing sees provisional addresses, not zero. An undefined/`IMPORT` symbol yields `val=0, cnst=0`. Recursive equates → error 18 (`symbol <%s> recursively defined`, FATAL).

`cnst` starts at 1 and is cleared if either child is non-constant; it is *not* reset by the operator switch except in `SUB` (below) and `SYM`.

Arithmetic is done in `taddr` = **int32_t**:
- `DIV`/`MOD`: `if(rval==0){ general_error(41); val=0; }` else C `/` and `%` on signed int32 (truncation toward zero, sign of dividend for `%`).
- `LSH`: `val = lval<<rval` on signed int32.
- `RSH`: `val = lval>>rval` — **arithmetic** (signed) shift by default. `RSHU`: `val = ((utaddr)lval)>>rval` — logical. Which node is built is decided at *parse* time by `unsigned_shift` (`expr.c:311-312`): `new->type = unsigned_shift ? RSHU : RSH`. `unsigned_shift` defaults 0, set by `-unsshift` (`vasm.c:733-736`) or implicitly by `-devpac` (`cpus/m68k/cpu.c:4795`).
- `NOT` (`!`) yields `!lval`, i.e. 0/1 — note this uses plain C `!`, **not** `BOOLEAN`, so `!x` gives 1 while `x==y` gives -1.
- All comparisons and `&&`/`||` go through `BOOLEAN(x)` = `-(x)` → **-1 / 0**.

`simplify_expr()` (`expr.c:612-921`) folds constant subtrees eagerly at parse time (`parse_expr` calls it before returning) using the same operator table, and additionally rewrites `const - (sym - sym)` shapes:
```c
if(tree->type==SUB&&tree->right->type==SYM&&tree->left->type==SUB&&
   tree->left->left->type!=SYM&&tree->left->right->type==SYM){
  /* Rearrange nodes from "const-symbol-symbol", so that "symbol-symbol"
     is evaluated first, as it may yield a constant. */
  ...
}
```
Type promotion in folding: `NUM < HUG < FLT` (enum order), the wider wins; comparisons on HUG/FLT results demote back to `NUM`.

### 4.7 `SUB` and label-minus-label (`expr.c:944-966`)

```c
case SUB:
  find_base(tree->left,&lsym,sec,pc);
  find_base(tree->right,&rsym,sec,pc);
  if(cnst==0&&lsym!=NULL&&rsym!=NULL&&LOCREF(rsym)){
    if(LOCREF(lsym)&&lsym->sec==rsym->sec){
      /* l2-l1 is constant when both have a valid symbol-base, and both
         symbols are LABSYMs from the same section, e.g. (sym1+x)-(sym2-y) */
      cnst=1;
    }else if(rsym->sec==sec&&(EXTREF(lsym)||LOCREF(lsym))){
      if((rsym->flags&ABSLABEL)&&(lsym->flags&ABSLABEL))
        cnst=1;  /* constant, when labels are from two ORG sections */
      else{
        /* prepare a value which works with REL_PC */
        val=(pc-rval+lval-(lsym->sec?lsym->sec->org:0));
        break;
      }
    }
  }
  val=(lval-rval);
  break;
```
So **label − label in the same section is constant**, and label − label across two absolute (`ORG`) sections is also constant. `LOCREF(s)` is `s->type==LABSYM && !(s->flags&WEAK)`; `EXTREF(s)` is `s->type==IMPORT || (s->flags&WEAK)` (`symbol.h:50-51`).

### 4.8 `find_base()` (`expr.c:1296-1396`)

Return codes (`expr.h:60-63`): `BASE_ILLEGAL 0`, `BASE_OK 1`, `BASE_PCREL 2`, `BASE_NONE -1`.

`find_base` first tries `find_abs_base()`, which walks the tree looking for exactly one `ABSLABEL` (label from an `ORG`/absolute section). If it finds one → `BASE_OK` (or `BASE_ILLEGAL` for an EXTREF); if it finds none but the walk succeeded → `BASE_NONE` ("all labels are absolute"); two absolute bases → `*base=NULL` and `BASE_NONE`. Otherwise `_find_base()`:
- `SYM` → recurse through EXPRESSION symbols, else return that symbol with `BASE_OK`.
- `ADD` → `BASE_OK` if one side evaluates constant and the other has a base.
- `SUB` → `BASE_OK` if the right side is constant and the left has a base; else if both sides have bases and the right is a `LOCREF` in the current section → `BASE_PCREL`.
- otherwise `BASE_ILLEGAL`.

For `-Fbin` no relocations are emitted, but `find_base` still drives the `SUB` constancy logic above and the error-38 checks in `space_size` and `fix_labels`.

---

## 5. Symbols (`symbol.c`, `symbol.h`, `symtab.c`)

### 5.1 Types and flags (`symbol.h:8-47`)

```c
#define LABSYM 1
#define IMPORT 2
#define EXPRESSION 3

#define TYPE_MASK 7 /* TYPE_UNKNOWN/OBJECT/FUNCTION/SECTION/FILE */
#define EXPORT (1<<3)   #define INEVAL (1<<4)   #define COMMON (1<<5)
#define WEAK   (1<<6)   #define LOCAL  (1<<7)   #define VASMINTERN (1<<8)
#define PROTECTED (1<<9) #define REFERENCED (1<<10) #define ABSLABEL (1<<11)
#define EQUATE (1<<12)  #define REGLIST (1<<13) #define USED (1<<14)

struct symbol { struct symbol *next; int type; uint32_t flags; char *name;
                expr *expr; expr *size; section *sec; taddr pc; taddr align;
                uint32_t idx; };
```

### 5.2 Definition and redefinition rules

`new_abs()` (`symbol.c:217-246`):
```c
if (new) {
  if (new->flags&EQUATE)  general_error(67,name); /* repeatedly defined symbol (error) */
  if (new->type!=IMPORT && new->type!=EXPRESSION) general_error(5,name); /* symbol redefined (warning) */
  add=0;
} else { ...allocate...; add=1; }
new->type = EXPRESSION; new->sec = 0; new->expr = tree;
```
`new_equate()` (`symbol.c:249-257`):
```c
check_symbol(name);            /* error 67 if already defined and not IMPORT */
sym = new_abs(name,tree);
sym->flags |= EQUATE;          /* not allowed to change this absolute symbol */
```

Therefore:
- **`equ` and `=` are identical** in mot syntax (both call `new_equate`, `syntax.c:1737-1760`) and are **not** redefinable — a second definition raises error 67 (`repeatedly defined symbol`) twice over (once from `check_symbol`, once from the `EQUATE` test).
- **`set` calls `new_abs` directly** (`syntax.c:1761-1765`) — no `check_symbol`, no `EQUATE` flag → freely redefinable, including redefining a previous `set`. Redefining a *label* with `set` gives warning 5.
- `rs`/`so`/`fo` label forms call `new_equate` via `new_setoffset` (`syntax.c:233-235`).
- `label REG …`/`FREG …` call `new_equate` (`cpus/m68k/cpu.c:5486,5496`).
- `label EQUR Rn` creates a **register symbol** (`new_regsym`), not a normal symbol; `-regsymredef` allows redefinition, otherwise error 58.

`new_labsym()` (`symbol.c:280-346`):
```c
if (chklabels) { /* -chklabels: warn 39/40 if name matches a mnemonic/directive */ }
if (!sec) { sec = default_section(); if (!sec) { general_error(3); return new_import(name); } }
sec->flags |= HAS_SYMBOLS;
if (sec->flags&LABELS_ARE_LOCAL) name = make_local_label(sec->name,strlen(sec->name),name,strlen(name));
if (new = find_symbol(name)) {
  if (new->type!=IMPORT) { symbol *old = new; new = mymalloc(...); *new = *old; general_error(5,name); }
  add = 0;
} else { ...; add = 1; }
new->type = LABSYM; new->sec = sec; new->pc = sec->pc;
if (add) { add_symbol(new); new->flags=0; new->size=0; new->align=0; }
if (*name != ' ') last_global_label = new->name;
if (sec->flags & ABSOLUTE) new->flags |= ABSLABEL; else new->flags &= ~ABSLABEL;
```
Note the duplicate-label path: it warns 5 and then works on a **copy** that is never added to the hash table, so the original symbol keeps its old address and the duplicate silently becomes orphaned. `LABELS_ARE_LOCAL` is never set by the mot module.

`check_symbol()` (`symbol.c:162-174`) errors 67 unless the existing symbol is an `IMPORT`.

### 5.3 Case sensitivity

`symtab.c:90-155`: `find_name`/`find_namelen` dispatch to the `_nc` variants when the global `nocase` is set. `add_hashentry` likewise hashes case-insensitively when `nocase` (`symtab.c:57`). Default `nocase = 0` → **symbol names are case-sensitive**; `-nocase` makes them insensitive.

Independently and unconditionally:
- **Mnemonics** are looked up with `find_namelen_nc` (`atom.c:29`) — always case-insensitive.
- **Directives** are looked up with `find_namelen_nc` (`syntax.c:1608`) — always case-insensitive.
- **Macro names** use `nocase_macros` (`parse.c:7`, default 0; set to 1 by `-phxass`) — case-sensitive by default.

### 5.4 Local labels

Naming scheme (`symbol.c:186-214`):
```c
int is_local_label(char *name) { return *name == ' '; }

char *make_local_label(char *glob,int glen,char *loc,int llen)
/* construct a local label of the form:
   " " + global_label_name + " " + local_label_name */
{
  char *name,*p;
  if (glen == 0) { glob = last_global_label; glen = strlen(last_global_label); }
  p = name = mymalloc(llen+glen+3);
  *p++ = ' ';
  if (glen) { memcpy(p,glob,glen); p += glen; }
  *p++ = ' ';
  memcpy(p,loc,llen);
  *(p + llen) = '\0';
  return name;
}
```
`last_global_label` starts as `emptystr` (`symbol.c:10`).

Recognition (`syntax.c:2169-2200`):
```c
char *get_local_label(char **start)
/* Motorola local labels start with a '.' or end with '$': "1234$", ".1" */
{
  char *s,*p,*name;
  int globlen = 0;

  name = NULL;
  s = *start;
  p = skip_local(s);

  if (p!=NULL && *p=='\\' && ISIDSTART(*s) && *s!=local_char && *(p-1)!='$') {
    /* skip local part of global\local label */
    globlen = p - s;
    s = p + 1;
    p = skip_local(s);
  }

  if (p!=NULL && p>(s+1)) {  /* identifier with at least 2 characters */
    if (*s == local_char) {
      /* .label */
      name = make_local_label(*start,globlen,s,p-s);
      *start = skip(p);
    }
    else if (*(p-1) == '$') {
      /* label$ */
      name = make_local_label(*start,globlen,s,(p-1)-s);
      *start = skip(p);
    }
  }
  return name;
}
```
with
```c
static char *skip_local(char *p)
{
  char *s;
  if (ISIDSTART(*p) || isdigit((unsigned char)*p)) {  /* may start with digit */
    s = p++;
    while (ISIDCHAR(*p)) p++;
    p = CHKIDEND(s,p);
  }
  else p = NULL;
  return p;
}
```
(`syntax.c:1653-1667` is the near-identical `parse_local_label`, which is currently unused.)

Exact rules:
- `local_char` is `'.'` by default, `'_'` with `-localu` (`syntax.c:53`, `:2295-2298`).
- A local label must be **at least 2 characters** (`p > s+1`). `.1` qualifies; `.` alone does not; `$` alone does not.
- `.label` → the mangled name **keeps the leading dot**: `" " + <last global> + " " + ".label"`. Example: under global `main`, `.loop` becomes the string `" main .loop"` (leading space, `main`, space, `.loop`).
- `label$` → the mangled name **drops the trailing `$`**: `" main label"`.
  Consequently `.loop` and `loop$` are two *different* symbols.
- `global\local` (PhxAss form) explicitly names the scope: `globlen` is the length of the part before the backslash and that text is used instead of `last_global_label`. It only triggers when the first part starts with `ISIDSTART`, is not itself local, and doesn't end in `$`. Note this conflicts with macro-argument `\` expansion, so it does not work inside macros.
- Locals starting with a digit are allowed (`skip_local` explicitly permits a leading digit) — `1234$` works.

**Scope reset**: `last_global_label` is assigned **only** in `new_labsym` when `*name != ' '` (`symbol.c:337-338`), and by `set_last_global_label()` which the mot module calls only from `inline`/`einline` (`syntax.c:1387-1417`, format `"=%06d"`). It is **not** reset by `section`, `org`, `include`, `macro`, or `rept`. Equates (`equ`/`=`/`set`) do **not** change it, because they never call `new_labsym`.

`inline` pushes a synthetic global label `=NNNNNN` onto a 100-deep stack, so all locals defined between `inline` and `einline` are isolated; `einline` restores the previous one.

### 5.5 `\@` macro-unique labels

Handled in `expand_macro` (`syntax.c:1992-2047`): `\@` expands to `snprintf(d,dlen,"_%06lu",unique_id)` where `unique_id = src->id` — the per-`source` counter assigned in `new_source` (`vasm.c:944,970`, a monotonically increasing `static unsigned long id`, starting at 0 for the main file). Extras:
- `\@!` — push the id onto a 100-entry stack.
- `\@?` — insert the id *below* the top of the stack.
- `\@@` — expand the id from the top of the stack, then pop it.

`\@` expansion only happens when `cur_src->num_params >= 0` (i.e. inside a macro; see `read_next_line`, `parse.c:1028-1029`). A `rept` block nested inside a macro copies `num_params` but gets a **new** `src->id`, so `\@` inside such a rept differs from `\@` in the enclosing macro body.

### 5.6 Internal symbols

- `__VASM` — `internal_abs(vasmsym_name)` (`vasm.c:762`), value 0; the CPU module updates it.
- `__RS` / `__SO` — `__SO` is a second hash entry pointing at the *same* symbol as `__RS` (`syntax.c:2227-2228`, `refer_symbol`).
- `__FO` — separate.
- `__LINE__` — updated per source line unless `-phxass`/`-devpac` (`syntax.c:1706-1707`, `set_internal_abs(line_name,real_line())`).
- `NARG` — not a symbol; resolved inline in `primary_expr` (`expr.c:206-213`) with a **case-sensitive `strcmp`**, so only uppercase `NARG` works (`syntax.h:28`). Value is `cur_src->num_params`.
- `CARG` (`syntax.h:31`) — a real internal absolute symbol, reset to 1 on each macro invocation (`my_exec_macro`, `syntax.c:1928-1936`).
- `REPTN` (`syntax.h:34`) — internal absolute; `-1` outside a repeat, iteration counter inside (`parse.c:780-782,921`).

---

## 6. Motorola syntax module (`syntax/mot/syntax.c`)

Module identity: `"vasm motorola syntax module 3.9e (c) 2002-2017 Frank Wille"` (`syntax.c:15`), `commentchar = ';'` (`:17`).

### 6.1 Full `directives[]` table (`syntax.c:1422-1589`)

`D=1` (available in `-devpac`), `P=2` (available in `-phxass`), `0` = neither. The registration filter is `if ((directives[i].avail & avail) == avail)` with `avail = 1` for devpac, `2` for phxass, **`0` (all directives registered) in the default mode** (`syntax.c:2210-2220`).

| name | avail | handler | semantics |
|---|---|---|---|
| `org` | P\|D | `handle_org` | `* = *+n` → `ds.b n`; else `start_rorg(n)` if current section is relocatable, else switch to a new absolute `seg<hex>` section |
| `rorg` | P\|D | `handle_rorg` | `ROFFS` atom: pad up to `sec->org + expr` |
| `section` | P\|D | `handle_section` | `section <name>[,<type>[,<mem>]]`; type guessed from name when omitted |
| `offset` | P\|D | `handle_offset` | switch to an unallocated `OFFSET%06lu` section at the given offset (`-1` = keep last) |
| `code`,`cseg`,`text` | P\|D / P / P\|D | `handle_csec` | section `CODE`, attr `acrx` |
| `data`,`dseg` | P\|D / P | `handle_dsec` | section `DATA`, attr `adrw` |
| `bss` | P\|D | `handle_bss` | section `BSS`, attr `aurw` |
| `code_c`/`code_f`/`data_c`/`data_f`/`bss_c`/`bss_f` | P\|D | `handle_codec` etc. | same, with AmigaDOS `memattr` 2 (chip) / 4 (fast) |
| `public`,`xdef`,`xref`,`xref.l`,`nref`,`entry`,`extrn`,`global`,`import`,`export` | mixed | `handle_global` | mark comma-separated symbols `EXPORT` (all of these are aliases; `xref` really does set EXPORT) |
| `weak` | 0 | `handle_weak` | mark symbols `WEAK` |
| `comm` | 0 | `handle_comm` | `comm sym,size` → IMPORT+COMMON, size expr, `align = 4` |
| `load`,`jumperr`,`jumpptr` | P / 0 / 0 | `handle_dummy_expr` | parse and discard an expression; warning 11 |
| `mask2` | 0 | `eol` | no-op, checks end of line |
| `cnop` | P\|D | `handle_cnop` | `cnop <offset>,<align>` — see §6.5 |
| `align` | 0 | `handle_align` | `align <n>` → alignment `1<<n`, offset 0 |
| `even` | P\|D | `handle_even` | alignment 2, offset 0 |
| `odd` | 0 | `handle_odd` | alignment 2, **offset 1** |
| `dc` | P\|D | `handle_d16` | default word |
| `dc.b`/`dc.w`/`dc.l`/`dc.q` | P\|D (q: P) | `handle_d8/16/32/64` | integer data |
| `dc.s`/`dc.d`/`dc.x` | P\|D | `handle_f32/f64/f96` | IEEE single/double/extended |
| `ds` | P\|D | `handle_spc16` | default word |
| `ds.b/.w/.l/.q/.s/.d/.x` | P\|D (q: P) | `handle_spc8/16/32/64/32/64/96` | zero-filled space, count × unit |
| `dcb` | P\|D | `handle_blk16` | `dcb <count>[,<fill>]` |
| `dcb.b/.w/.l/.q/.s/.d/.x` | P\|D (q: P) | `handle_blk8/16/32/64/32/64/96` | filled block |
| `blk`, `blk.b…blk.x` | P | `handle_blk*` | aliases for `dcb*` |
| `dr`,`dr.b`,`dr.w`,`dr.l` | 0 | `handle_reldata*` | m68k only: emit `<expr> - <here>` PC-relative data |
| `end` | P\|D | `handle_end` | set `parse_end` — every subsequent line is skipped |
| `fail` | P\|D | `handle_fail` | `ASSERT` atom with no expression → error 19 `fail: %s` at assemble time |
| `idnt`,`ttl`,`module` | P\|D | `handle_idnt` | set the module/file name (`setfilename`) |
| `list`/`nolist` | P\|D | `handle_list`/`handle_nolist` | listing on/off |
| `plen` | P\|D | `handle_plen` | listing lines/page, min 12 |
| `llen`,`spc` | P\|D | `handle_dummy_cexpr` | parse a constexpr, warn 11 |
| `page`/`nopage` | P\|D | `handle_page`/`handle_nopage` | `listformfeed = 1 / 0` |
| `output` | P\|D | `handle_output` | set output name; a leading `.` replaces the input file's extension |
| `symdebug` | P | `eol` | no-op |
| `dsource` | P | `handle_dsource` | set debug source name |
| `debug` | P | `handle_debug` | `LINE` atom with the given line number |
| `comment` | P\|D | `handle_comment` | ignored, except `COMMENT HEAD=<expr>` → `" TOSFLAGS"` symbol |
| `incdir` | P\|D | `handle_incdir` | append comma-separated include paths |
| `include` | P\|D | `handle_include` | include a source file |
| `incbin`,`image` | P\|D / 0 | `handle_incbin` | **`incbin <file>` only — no offset/length arguments in 1.7h** (`syntax.c:976-983` calls `include_binary_file(name,0,0)`) |
| `rept` | P\|D | `handle_rept` | begin repeat block, count = constexpr |
| `endr` | P\|D | `handle_endr` | error 12 if reached standalone |
| `macro` | P\|D | `handle_macro` | `macro <name>` form (name in the operand field) |
| `endm` | P\|D | `handle_endm` | error 12 if reached standalone |
| `mexit` | P\|D | `handle_mexit` | leave the current macro immediately |
| `rem`/`erem` | P | `handle_rem`/`handle_erem` | block comment: `new_repeat(0,NULL,erem_dirlist)` skips to `erem` with count 0 |
| `ifb`/`ifnb` | 0 | `handle_ifb`/`handle_ifnb` | true if the operand field is (not) empty |
| `ifc`/`ifnc` | P\|D | `handle_ifc`/`handle_ifnc` | string compare of two `parse_name()` operands |
| `ifd`/`ifnd` | P\|D | `handle_ifd`/`handle_ifnd` | symbol defined (`find_symbol` and `type != IMPORT`) |
| `ifmacrod`/`ifmacrond` | 0 | | macro defined / not defined |
| `ifeq`,`ifne`,`ifgt`,`ifge`,`iflt`,`ifle` | P\|D | `ifexp(s,0..5)` | `val==0`, `!=0`, `>0`, `>=0`, `<0`, `<=0` |
| `ifmi`,`ifpl` | 0 | `handle_iflt`,`handle_ifge` | aliases |
| `if` | P | `handle_ifne` | `val != 0` |
| `else`,`elseif` | P\|D | `handle_else` | both are plain `else` (`elseif` takes **no** condition) |
| `endif`,`endc` | P\|D | `handle_endif` | close conditional |
| `rsreset`,`clrso` | P\|D / P | `handle_rsreset` | `__RS = 0` |
| `rsset`,`setso` | P\|D / P | `handle_rsset` | `__RS = <constexpr>` |
| `clrfo`/`setfo` | P | | `__FO = 0` / `= <constexpr>` |
| `rs`,`rs.b/.w/.l/.q/.s/.d/.x` | P\|D (q: P) | `handle_rs8..96` | advance `__RS` by `n × size` (sizes 1/2/4/8/4/8/12) |
| `so`,`so.*` | P | `handle_rs*` | alias of `rs` (same `__RS` symbol) |
| `fo`,`fo.*` | P | `handle_fo*` | decrement `__FO` |
| `cargs` | P\|D | `handle_cargs` | `cargs [#<offset>,]<sym>[.<size>],…` — offset defaults to 4, sizes: `.l`→4, `.b`/`.w`/missing→2 |
| `echo` | P | `handle_printt` | print string(s) |
| `printt` | 0 | `handle_printt` | same |
| `printv` | 0 | `handle_printv` | print `$hex sdec "asc" %bin` per expression, 32-bit fields |
| `auto` | 0 | `handle_noop` | warn 11 |
| `inline`/`einline` | P | | isolated local-label block |

**Not present in 1.7h mot:** `assert`, `equr`/`reg`/`opt`/`machine`/`fpu` (these are *CPU-module* directives, see §6.9), `dcb` string forms, `incbin` with offset, `even` with an argument, `struct`/`endstruct`.

`check_directive()` (`syntax.c:1597-1612`) — the directive name scan explicitly allows `.`, which is how `dc.b` is matched as one token even when `dot_idchar` is off:
```c
s = skip(*line);
if (!ISIDSTART(*s)) return -1;
name = s++;
while (ISIDCHAR(*s) || *s=='.') s++;
if (!find_namelen_nc(dirhash,name,s-name,&data)) return -1;
*line = s;
return data.idx;
```
Then `handle_directive` calls `directives[idx].func(skip(line))` (`syntax.c:1617-1626`).

### 6.2 `-spaces` — exactly what changes

Three places, all keyed on `allow_spaces` (`syntax.c:50`, set by `-spaces` at `:2287-2290` and implicitly by `-phxass` at `:2283`).

**(a) `exp_skip()` (`syntax.c:147-162`)** — installed as `EXPSKIP()` for `expr.c` via `syntax.h:21-22`, and called at the top of every `skip_operand()` iteration:
```c
char *exp_skip(char *s)
{
  if (allow_spaces) {
    char *start = s;
    s = skip(start);
    if (phxass_compat && s>start && *s=='*' && isspace((unsigned char )*(s-1)))
      *s = '\0';  /* rest of operand is ignored */
  }
  else if (isspace((unsigned char)*s)) {
    if (check_comm) comment_check(s);
    *s = '\0';  /* rest of operand is ignored */
  }
  return s;
}
```
Without `-spaces`, the **first whitespace character inside an operand destructively terminates the line buffer** — everything after it is a comment. `dc.w 1 + 2` emits one word `1`. With `-spaces`, whitespace is merely skipped.

**(b) `eol()` (`syntax.c:107-118`)**:
```c
void eol(char *s)
{
  if (allow_spaces) { s = skip(s); if (!ISEOL(s)) syntax_error(6); }
  else { if (!ISEOL(s) && !isspace((unsigned char)*s)) syntax_error(6); }
}
```

**(c) the operand-splitting loop in `parse()`** — see §6.4. Without `-spaces` the comma must be immediately adjacent; with `-spaces` `skip()` is applied on both sides.

`-warncomm` (`check_comm`) only adds warning 1018 (`check comment`) in the non-`-spaces` path (`syntax.c:96-103`).

### 6.3 Label parsing, colons, comments

`parse_labeldef()` (`parse.c:305-333`):
```c
char *s = *line;
char *labname;

if (isspace((unsigned char )*s)) {
  s = skip(s);
  needcolon = 1;  /* colon required, when label doesn't start at 1st column */
}
if (labname = parse_symbol(&s)) {
  s = skip(s);
  if (*s == ':') { s++; needcolon = 0; }
  if (needcolon) { myfree(labname); labname = NULL; }
  else *line = s;
}
return labname;
```
called as `parse_labeldef(&s,0)` from `parse()` (`syntax.c:1714,1731`). So: **a label starting in column 1 needs no colon; a label not in column 1 requires one.** A trailing colon is always consumed. `parse_symbol` (`parse.c:293-302`) tries `get_local_label` first, then `parse_identifier`.

Identifier character classes (`syntax/mot/syntax.h:8-14`, `syntax.c:121-144`):
```c
#define ISIDSTART(x) ((x)=='.'||(x)=='@'||(x)=='_'||isalpha((unsigned char)(x)))
#define ISIDCHAR(x) isidchar(x)
#define ISBADID(p,l) ((l)==1&&(*(p)=='.'||*(p)=='@'||*(p)=='_'))
#define ISEOL(p) (*(p)=='\0'||iscomment(p))
#define CHKIDEND(s,e) chkidend((s),(e))

int isidchar(char c)
{
  if (isalnum((unsigned char)c) || c=='_' || c=='$' || c=='%') return 1;
  if (dot_idchar && c=='.') return 1;
  if (phxass_compat && (unsigned char)c>=0x80) return 1;
  return 0;
}

char *chkidend(char *start,char *end)
{
  if (dot_idchar && (end-start)>2 && *(end-2)=='.') {
    char c = tolower((unsigned char)*(end-1));
    if (c=='b' || c=='w' || c=='l')
      return end - 2;	/* .b/.w/.l extension is not part of identifier */
  }
  return end;
}
```
Note `$` and `%` are ordinary identifier characters (so `foo$bar` is one identifier, and `label$` only becomes local because `get_local_label` checks the *last* char). Dots are **not** identifier characters unless `-ldots`/`-devpac`.

Comments:
- `;` anywhere → `iscomment()` (`syntax.c:80-92`) returns true; `ISEOL` therefore treats it as end of line.
- `*` at the start of the operation field: `parse()` at `syntax.c:1798-1801`:
  ```c
  s = skip(s);
  if (*s=='\0' || *s=='*' || *s==commentchar)
    continue;
  ```
  This runs *after* the label field, so `*` need not be in column 1 — it just has to be the first non-blank of the operation field. In an operand, `*` is the current-PC symbol.
- Anything after the operands, separated by a blank, is a comment — that is the `exp_skip` `*s='\0'` behaviour above (non-`-spaces` mode only).
- `-phxass` extends `iscomment` so a `*` preceded by a blank starts a comment even inside operands.

### 6.4 Operand splitting — quoted

`skip_operand()` (`syntax.c:165-194`):
```c
char *skip_operand(char *s)
{
  int par_cnt = 0;
  char c;

  for (;;) {
    s = exp_skip(s);
    c = *s;

    if (START_PARENTH(c)) {
      par_cnt++;
    }
    else if (END_PARENTH(c)) {
      if (par_cnt>0)
        par_cnt--;
      else
        syntax_error(3);  /* too many closing parentheses */
    }
    else if (c=='\'' || c=='\"')
      s = skip_string(s,c,NULL) - 1;
    else if (!c || (par_cnt==0 && (c==',' || iscomment(s))))
      break;

    s++;
  }

  if (par_cnt != 0)
    syntax_error(4);  /* missing closing parentheses */
  return s;
}
```
`START_PARENTH(x)` is `((x)=='(')` and `END_PARENTH(x)` is `((x)==')')` (`vasm.h:51-57`; the m68k module does not override them, so `[`/`]` are *not* grouping characters unless `-conv-brackets` rewrites them earlier).

The caller in `parse()` (`syntax.c:1835-1865`):
```c
    /* read operands, terminated by comma (unless in parentheses)  */
    op_cnt = 0;
    if (!ISEOL(s)) {
      while (op_cnt < MAX_OPERANDS) {
        op[op_cnt] = s;
        s = skip_operand(s);
        op_len[op_cnt] = s - op[op_cnt];
        op_cnt++;

        if (allow_spaces) {
          s = skip(s);
          if (*s != ',')
            break;
          else
            s = skip(s+1);
        }
        else {
          if (*s != ',') {
            if (check_comm)
              comment_check(s);
            break;
          }
          s++;
        }
        if (ISEOL(s)) {
          syntax_error(6);  /* garbage at end of line */
          break;
        }
      }
      eol(s);
    }
```
`MAX_OPERANDS` is 6 for m68k. Note `op_len` is a byte length into the (mutated) line buffer — `exp_skip` may already have written a `'\0'` into it.

`skip_string()` (`parse.c:200-230`) counts characters, honours `escape()`, and treats a doubled delimiter as one literal character; it errors 6 (`%c expected`) if unterminated.

### 6.5 Data, space and alignment directives

`handle_data()` (`syntax.c:587-622`):
```c
static void handle_data(char *s,int size)
{
  /* size is negative for floating point data! */
  for (;;) {
    char *opstart = s;
    operand *op;
    dblock *db = NULL;

    if (OPSZ_BITS(size)==8 && (*s=='\"' || *s=='\'')) {
      if (db = parse_string(&opstart,*s,8)) {
        add_atom(0,new_data_atom(db,1));
        s = opstart;
      }
    }
    if (!db) {
      op = new_operand();
      s = skip_operand(s);
      if (parse_operand(opstart,s-opstart,op,DATA_OPERAND(size))) {
        atom *a;
        a = new_datadef_atom(OPSZ_BITS(size),op);
        if (!align_data)
          a->align = 1;
        add_atom(0,a);
      }
      else
        syntax_error(8);  /* invalid data operand */
    }

    s = skip(s);
    if (*s == ',')
      s = skip(s+1);
    else
      break;
  }
}
```
String handling detail: **only `dc.b` takes the string path**, and `parse_string()` (`parse.c:267-290`) returns `NULL` when the string is exactly **one** character long, so `dc.b "A"` falls through to the expression path and produces the same single byte. A zero-length string `""` produces a `DATA` atom of size 0. `dc.w "AB"` is *not* a string — it goes through `primary_expr`'s character-constant path and yields `0x4142`. Escape handling inside `dc.b` strings is `escape()` with `esc_sequences` off by default (§4.5). Note that within one `dc.b` line, string operands and expression operands can be freely mixed, and the loop's comma handling here uses `skip()` on both sides regardless of `-spaces` (but `skip_operand` already terminated the line at the first blank in non-`-spaces` mode).

`do_space` / `handle_space` / `handle_block` (`syntax.c:287-299, 719-730`):
```c
static void do_space(int size,expr *cnt,expr *fill)
{
  atom *a = new_space_atom(cnt,size>>3,fill);
  a->align = align_data ? DATA_ALIGN(size) : 1;
  add_atom(0,a);
}
static void handle_space(char *s,int size) { do_space(size,parse_expr_tmplab(&s),0); }
static void handle_block(char *s,int size)
{
  expr *cnt,*fill=0;
  cnt = parse_expr_tmplab(&s);
  s = skip(s);
  if (*s == ',') { s = skip(s+1); fill = parse_expr_tmplab(&s); }
  do_space(size,cnt,fill);
}
```
So `ds.X n` = `n` units of `size/8` bytes, zero-filled; `dcb.X n,f` = `n` units filled with `f` (big-endian, `copy_cpu_taddr`).

Alignment (`syntax.c:667-716`):
```c
static void do_alignment(taddr align,expr *offset,size_t pad,expr *fill)
{
  atom *a = new_space_atom(offset,pad,fill);
  a->align = align;
  add_atom(0,a);
}

static void handle_cnop(char *s)
{
  expr *offset;
  taddr align=1;

  offset = parse_expr_tmplab(&s);
  s = skip(s);
  if (*s == ',') { s = skip(s+1); align = parse_constexpr(&s); }
  else syntax_error(9);  /* , expected */

#ifdef VASM_CPU_M68K
  /* align with NOP instructions in an M68k code section */
  if (!devpac_compat && align>3 &&
      (current_section==NULL || strchr(current_section->attr,'c')!=NULL))
    do_alignment(align,offset,2,number_expr(0x4e71));
  else
#endif
    do_alignment(align,offset,1,NULL);
}

static void handle_align(char *s) { do_alignment(1<<parse_constexpr(&s),number_expr(0),1,NULL); }
static void handle_even(char *s)  { do_alignment(2,number_expr(0),1,NULL); }
static void handle_odd(char *s)   { do_alignment(2,number_expr(1),1,NULL); }
```
Two consequences you must reproduce:
1. `cnop 0,4` (or any align > 3) **in a section whose attr contains `c`, or when no section exists yet**, and not in `-devpac` mode, creates a SPACE atom with `size=2, fill=0x4E71, space=0`. `fwpcalign` then pads with `4E71` words (§3.2 item 7). With `align <= 3`, or in a data/bss section, or under `-devpac`, padding is zero bytes.
2. When the cnop *offset* is non-zero and the NOP path was taken, `space != 0`, so the alignment padding uses `sec->pad` (zeros) and the offset is then emitted as `offset × 2` bytes of `4E71`. `cnop 2,8` in a code section therefore emits 4 bytes of `4E71` after aligning to 8 with zeros. This is surprising but it is what the code does.
3. `align n` means `1<<n` (so `align 2` → 4-byte alignment), padding with zeros, and it is **not** limited to code sections.

`new_setoffset_size` (`syntax.c:199-241`) implements the `rs`/`so`/`fo` counters, including the `-align` even-alignment path:
```c
if (align_data && size>1) {
  utaddr dalign = DATA_ALIGN((int)size*8) - 1;
  old = make_expr(BAND, make_expr(dir>0?ADD:SUB,sym->expr,number_expr(dalign)),
                  number_expr(~dalign));
  simplify_expr(old);
} else old = sym->expr;
new = make_expr(dir>0?ADD:SUB,old,new);
```
`new_setoffset` (`:246-284`) reads the extension from `*(start+2)`: `b`→1, `w`→2, `l`/`s`→4, `q`/`d`→8, `x`→12, **missing → 2**.

### 6.6 Macros

Definition. Two forms, both ending at `endm` (`endm_dirlist`, `syntax.c:32-34`):
- `<name> macro` — label-field form, `syntax.c:1776-1786`. The label field is **re-parsed from the raw line** with `parse_identifier`, so the macro name may not be a local label:
  ```c
  else if (!strnicmp(s,"macro",5) &&
           (isspace((unsigned char)*(s+5)) || *(s+5)=='\0')) {
    /* reread original label field as macro name, no local macros */
    s = line;
    myfree(labname);
    if (!(labname = parse_identifier(&s))) ierror(0);
    new_macro(labname,endm_dirlist,NULL);
    myfree(labname);
    continue;
  }
  ```
- `macro <name>` — directive form, `handle_macro` (`syntax.c:998-1004`), name via `parse_name()`.

Both pass `args = NULL`, and `SKIP_MACRO_ARGNAME(p)` is `(NULL)` in mot (`syntax.h:38`), so **named macro parameters are not supported** — positional only. `new_macro` (`parse.c:505-580`) errors 51/52 if the name collides with a mnemonic or directive, and records `m->text = cur_src->srcptr` (the character after the `macro` line).

Body capture: `read_next_line` (`parse.c:958-1022`) scans forward for a directive from `enddir_list` using `dirlist_match` (case-insensitive, requires a following whitespace char, `parse.c:398-412`). It skips over quoted strings while scanning, and it errors 26 (`macro definition inside macro`) if a `macro` appears while already inside one. Missing `endm` → error 25 (FATAL).

Invocation. `execute_macro` (`parse.c:600-740`) is called from `parse()` (`syntax.c:1832`) **after** the mnemonic and its qualifiers have been split off:
```c
    s = parse_instruction(s,&inst_len,ext,ext_len,&ext_cnt);
    if (!isspace((unsigned char)*s) && *s!='\0')
      syntax_error(2);  /* no space before operands */
    s = skip(s);
    if (execute_macro(inst,inst_len,ext,ext_len,ext_cnt,s))
      continue;
```
So macros are looked up **after** directives and after `parse_cpu_special`, and a macro invocation may carry a `.size` qualifier which becomes `\0`.

Argument splitting: `parse_macro_arg` (`syntax.c:1887-1924`):
```c
char *parse_macro_arg(struct macro *m,char *s,
                      struct namelen *param,struct namelen *arg)
{
  arg->len = 0;  /* no argument reference by keyword */
  param->name = s;

  if (*s == '<') {
    /* macro argument enclosed in < ... > */
    param->name++;
    while (*++s != '\0') {
      if (*s == '>') {
        if (*(s+1) == '>') {
          /* convert ">>" into a single ">" by shifting the whole line buffer */
          char *p;
          for (p=s+1; *p!='\0'; p++) *(p-1) = *p;
          *(p-1) = '\0';
        }
        else { param->len = s - param->name; s++; break; }
      }
    }
  }
  else if (*s=='\"' || *s=='\'') {
    s = skip_string(s,*s,NULL);
    param->len = s - param->name;
  }
  else {
    s = skip_operand(s);
    param->len = trim(s) - param->name;
  }
  return s;
}
```
- `<…>` quoting includes the delimiters' contents only; `>>` inside becomes a literal `>` by shifting the line buffer left.
- A quoted string keeps its quotes in the parameter text.
- Otherwise `skip_operand()` is used (so commas at paren-depth 0 separate, and in non-`-spaces` mode a blank terminates), then trailing blanks are trimmed (`trim`, `supp.c`).

Separators use the defaults `MACRO_PARAM_SEP(p)` = `(*p==',' ? skip(p+1) : NULL)` (`parse.h:86-88`). `maxmacparams` is 9 by default, 35 with `-allmp`/`-devpac`/`-phxass` (`syntax.c:2230`; `MAXMACPARAMS 35` in `syntax.h:37`). Exceeding it → error 27. Missing parameters default to `emptystr`; `m->defaults` is always NULL here (no named args), so unsupplied parameters are empty strings, and `parse.c:665-716` never assigns a positional slot when `param.len == 0` (an empty argument leaves the previous/default value but still advances `n`).

Recursion limit `maxmacrecurs` = 1000 (`parse.h:12-13`, `parse.c:9`), error 56. `mexit` → `leave_macro()` (`parse.c:743-751`), error 36 outside a macro.

Expansion escapes — `expand_macro()` (`syntax.c:1965-2137`), applied by `read_next_line` character-by-character while `cur_src->num_params >= 0`:

| escape | meaning | line |
|---|---|---|
| `\\` | literal `\` (doubled again if `esc_sequences`) | 1973 |
| `\@` | `_%06lu` of `src->id`; `\@!` push, `\@?` insert-below-top, `\@@` use-top-and-pop | 1992 |
| `\<sym>` | decimal value of an absolute EXPRESSION symbol; `\<$sym>` → uppercase hex; formats via `(unsigned long)(uint32_t)val` | 2049 |
| `\#` | number of parameters (`src->num_params`) | 2079 |
| `\?n` | length of parameter *n* (`\?0` = qualifier 0 length) | 2085 |
| `\.` | parameter `#CARG` | 2097 |
| `\+` | parameter `#CARG`, then CARG++ | 2102 |
| `\-` | parameter `#CARG`, then CARG-- | 2107 |
| `\0` | qualifier 0 — the `.size` suffix on the macro call | 2113 |
| `\1`..`\9` | parameters 1..9 | 2113 |
| `\a`..`\z` | parameters 10..35, **only when `maxmacparams > 9`** | 2122 |

Anything else after `\` is left untouched (`nc` stays 0 and the backslash is copied literally).

`narg` → `NARG` (uppercase, `syntax.h:28`).

### 6.7 `rept` / `endr`

`handle_rept` → `new_repeat((int)parse_constexpr(&s), rept_dirlist, endr_dirlist)` (`syntax.c:986-989`). `new_repeat` (`parse.c:446-457`) records the start pointer; `read_next_line` scans forward for `endr` while **counting nested `rept`** (`parse.c:962-986`, `rept_nest`), then `start_repeat` (`parse.c:766-804`) creates a new `source` over `[rept_start, rept_end)` with `repeat = rept_cnt` and `reptn = 0`. A count `<= 0` produces no source at all (`if (rept_cnt > 0)`), so `rept 0` skips the block entirely. Missing `endr` → error 32 (FATAL).

`rem`/`erem` reuses the same machinery with count 0 and a `NULL` rept-list, so nesting is not counted — the block ends at the first `erem`.

`REPTN` is updated at each wrap in `read_next_line` (`parse.c:917-922`).

### 6.8 Conditionals (`cond.c`, `syntax.c:1031-1182, 1709-1729`)

`MAXCONDLEV 63` (`cond.h:9`). State is a `char cond[MAXCONDLEV+1]` stack plus `ifnesting` for skipped blocks.

When `cond_state()` is false, `parse()` takes the skip path (`syntax.c:1709-1729`):
```c
    if (!cond_state()) {
      int idx;
      if (labname = parse_labeldef(&s,0)) myfree(labname);
      s = skip(s);
      idx = check_directive(&s);
      if (idx >= 0) {
        if (!strncmp(directives[idx].name,"if",2))
          cond_skipif();
        else if (directives[idx].func == handle_else)
          cond_else();
        else if (directives[idx].func == handle_endif)
          cond_endif();
      }
      continue;
    }
```
Note the `strncmp(name,"if",2)` test — it matches the *table entry name*, so `ifb`,`ifc`,`ifd`,`ifeq`,`ifmi`,`if`,… all count, and nothing else does. `ifmi`/`ifpl` match because their table names start with `if`. Labels on conditional lines are parsed and discarded (the doc warns not to use them).

`cond_check()` runs after the parse loop and errors 66 for any unclosed block (`syntax.c:1882`, `cond.c:30-34`).

`end_source()` restores `clev` to the level recorded when the source was entered (`vasm.c:987-994`), so an unterminated `if` inside an include does not leak.

### 6.9 CPU-module directives reached via `parse_cpu_special` / `parse_cpu_label`

`parse()` calls `parse_cpu_special(s)` **before** `handle_directive` (`syntax.c:1803-1805`). `cpus/m68k/cpu.c:5222-5449` recognises, case-insensitively: `near`/`.sdreg`, `far`, `initnear`, `basereg`, `endb`, `machine`, `mc680x0`, `mcf5xxx`, `cpu32`, `fpu`, **`opt`**, `optc` (phxass only). `opt` takes Devpac-style comma-separated options by default (`devpac_option`) or PhxAss single letters under `-phxass`; the whole rest of the line is consumed (`return skip_line(s)`).

`PARSE_CPU_LABEL` → `parse_cpu_label` (`cpu.c:5452-5504`) handles the label-field forms **`equr`**, `fequr`, **`reg`**, `equrl`, `freg`, `fequrl`. `equr` creates a register symbol; `reg`/`equrl` create an equate holding the register-list bitmask.

### 6.10 The label-field directive chain in `parse()` (`syntax.c:1731-1796`)

Order of tests after a label was parsed (all case-insensitive, all requiring a following blank unless noted):
1. `equ` → `new_equate(labname, parse_expr_tmplab(&s))`
2. `fequ.<x>` (checks `isspace(*(s+6))`) → `fequate` with `parse_expr_float`
3. `equ.<x>` — **phxass only**
4. `=` → `new_equate` (identical to `equ`); `=.` form is phxass-only float
5. `set` → `new_abs` (redefinable)
6. `rs`/`so` via `offs_directive` → `new_setoffset(..., __RS, +1)`
7. `fo` via `offs_directive` → `new_setoffset(..., __FO, -1)`
8. `ttl` → `setfilename(labname)`
9. `macro` → macro definition (label re-read as the name), `continue`
10. `PARSE_CPU_LABEL` → `equr`/`reg`/…
11. otherwise → `new_labsym(0,labname)` + `new_label_atom`

`offs_directive` (`syntax.c:1629-1637`):
```c
return !strnicmp(s,name,len) &&
       ((isspace((unsigned char)*d) || ISEOL(d)) ||
        (*d=='.' && (isspace((unsigned char)*(d+2))||ISEOL(d+2))));
```

---

## 7. Line reading (`parse.c:906-1106`, `vasm.c:857-983`)

**No line continuation exists in vasm 1.7h.** There is no backslash-newline or `&` continuation anywhere in `read_next_line`, `parse()`, or the docs. Each physical line is one logical line.

**Line length** is unbounded in practice: `source.linebuf` starts at `INITLINELEN 256` (`parse.h:8`, `vasm.c:973-974`) and **doubles** whenever it runs out:
```c
    if (nc < 0) {
      int offs = d - cur_src->linebuf;
      len += cur_src->bufsize;
      cur_src->bufsize += cur_src->bufsize;
      cur_src->linebuf = myrealloc(cur_src->linebuf,cur_src->bufsize);
      d = cur_src->linebuf + offs;
    }
```
The buffer starts with a sentinel `'\0'` at index 0 (`*d++ = 0;`, `parse.c:956`) so callers can look one character to the left; `parse()` receives `cur_src->linebuf + 1`. Listing lines are truncated to `MAXLISTSRC 120` (`vasm.h:146`).

**CRLF / CR** (`parse.c:1040-1055`):
```c
      if (*s == '\r') {
        if ((s>cur_src->srcptr && *(s-1)=='\n') ||
            (s<(srcend-1) && *(s+1)=='\n')) {
          /* ignore \r in \r\n and \n\r combinations */
          s++;
        }
        else {
          /* treat a single \r as \n */
          s++;
          break;
        }
      }
      else if (*s == '\n') { s++; break; }
```
So CRLF, LFCR, lone LF and lone CR all terminate a line, and the terminator is never stored in the buffer.

**Ctrl-Z** (`vasm.c:949-957`): the first `0x1a` byte in a source is replaced by `\n` and the rest of the file is discarded.

**File loading** (`vasm.c:892-919`): read in 64 KiB chunks with `fopen(...,"r")` (text mode — on Windows builds this already strips CR), then a `'\n'` and a `'\0'` are appended, so every source is guaranteed to end with a newline. An empty file becomes the single-character source `"\n"`.

**Tabs** are not expanded and are simply `isspace()` — they behave exactly like blanks everywhere (`skip`, `parse_labeldef`, `exp_skip`, `dirlist_match`).

**Case-insensitivity** — recap: mnemonics and directives always; macro names only with `nocase_macros`; symbols only with `-nocase`; section attributes/types compared with `strnicmp` in `read_sec_attr` and `stricmp` in `handle_section`.

`real_line()` (`parse.c:431-443`) walks up through macro sources to find the enclosing physical line, used for `__LINE__`.

---

## 8. Command-line options (`vasm.c:586-760`, plus module hooks)

### 8.1 Pre-scan (`vasm.c:589-602`)

Runs before anything else and blanks the matched arguments:
```c
for(i=1;i<argc;i++){
  if(argv[i][0]=='-'&&argv[i][1]=='F'){ output_format=argv[i]+2; argv[i][0]=0; }
  if(!strcmp("-quiet",argv[i])){ verbose=0; argv[i][0]=0; }
  if(!strcmp("-debug",argv[i])){ debug=1;   argv[i][0]=0; }
}
```
`output_format` defaults to `"test"` (`vasm.c:44`); the m68k `vasmm68k_mot` binary is normally built with `-DOUTFMT=bin`… but in this source tree the default is whatever `set_default_output_format()` was called with, so **always pass `-Fbin` explicitly**.

### 8.2 Core options (in dispatch order)

| option | effect | default | line |
|---|---|---|---|
| `-F<fmt>` | output module: `test`, `elf`, `bin`, `srec`, `vobj`, `hunk`, `aout`, `hunkexe`, `tos` | `test` | 590, 481-506 |
| `-quiet` | suppress banners and the `statistics()` dump | off (`verbose=1`) | 594, 76 |
| `-debug` | debug tracing, poisoned malloc | off | 598 |
| `-o <file>` | output name (separate argument required); duplicate → warning 28 | `a.out` | 620 |
| `-L <file>` | listing file, enables listing | off | 626 |
| `-Lnf` | no form feeds in listing | on | 634 |
| `-Lns` | no symbols in listing | off | 638 |
| `-Ll<n>` | lines per page | 40 | 642 |
| `-D<name>[=<expr>]` | define an absolute symbol (`new_abs`, value 1 if no `=`); accepts `-Dfoo` or `-D foo` | — | 646 |
| `-I<path>` | append include path; accepts `-Ipath` or `-I path` | — | 674 |
| `-depend=list\|make` | emit dependencies instead of output | off | 685 |
| `-unnamed-sections` | all section names become `""` | off | 695 |
| `-ignore-mult-inc` | skip repeated `include` of the same file | off | 699 |
| `-nocase` | case-insensitive symbol names | off | 703 |
| `-nosym` | strip the symbol table (no effect for `-Fbin`) | off | 707 |
| `-nowarn=<n>` | disable one warning number | — | 711 |
| `-w` | disable all warnings (`no_warn`) | off | 717 |
| `-maxerrors=<n>` | abort after n errors; `0` = unlimited | **5** (`error.c:28`) | 721 |
| `-pic` | check for position-independent code | off | 725 |
| `-maxmacrecurs=<n>` | macro recursion limit | 1000 | 729 |
| `-unsshift` | `>>` becomes a logical shift (`RSHU`) | off | 733 |
| `-chklabels` | warn when a label collides with a mnemonic/directive | off | 737 |
| `-esc` / `-noesc` | enable/disable backslash escapes in strings (checked *after* module args) | **off** (`parse.c:6`) | 747, 751 |
| `-x` | prefix match: disable auto-import of undefined symbols → error 22 instead | auto-import on | 755 |

Unrecognised → `general_error(14,argv[i])` printed as `error 15: unknown option <…>`.

Dispatch order for anything not matched above: `cpu_args()` → `syntax_args()` → `output_args()` → `-esc`/`-noesc`/`-x` → error. `-phxass` and `-devpac` are handled by `cpu_args` which deliberately `return 0` so `syntax_args` sees them too (`cpus/m68k/cpu.c:4777-4797`).

### 8.3 mot syntax-module options (`syntax.c:2259-2304`)

```c
-align     → align_data = 1
-allmp     → allmp = 1                       (35 macro params)
-devpac    → devpac_compat=1, align_data=1, esc_sequences=0, allmp=1,
             dot_idchar=1, warn_unalloc_ini_dat=1
-phxass    → _PHXASS_=2, phxass_compat=1, esc_sequences=1, nocase_macros=1,
             allow_spaces=1, allmp=1
-spaces    → allow_spaces = 1
-ldots     → dot_idchar = 1
-localu    → local_char = '_'
-warncomm  → check_comm = 1
```
`-devpac` and `-phxass` additionally restrict the directive table to the `D`/`P` subsets (§6.1).

### 8.4 m68k CPU options relevant to byte-exactness (`cpus/m68k/cpu.c:4772-4880+`)

`-m<cpu>` (e.g. `-m68000`, `-m68020`, `-mcf5206`), `-no-opt` (`clear_all_opts(); no_opt=1`), `-no-fpu`, `-sdreg=<n>`, `-gas` (changes `commentchar` to `|`!), `-sgs`, `-sc`, `-rangewarnings`, `-conv-brackets`, `-regsymredef`, `-elfregs`, `-guess-ext`, `-showcrit`, `-showopt`, and per-optimisation `-opt-movem`/`-opt-pea`/`-opt-clr`/… . `-no-opt` is the one to reach for when you want sizes to be purely syntactic.

### 8.5 `-Fbin` module options

Only `-cbm-prg` (`output_bin.c:93-100`).

### 8.6 Include-path search order

Built in this order, and searched in this order by `locate_file` (`vasm.c:826-855`, `1186-1228`):
1. `"."` — added by `init_main()` before argument parsing (`vasm.c:527`).
2. every `-I` path, in command-line order.
3. the directory of the input source file, if the input name contains `/`, `\` or `:` (`set_input_name`, `vasm.c:537-556`).
4. every `incdir` directive path, in the order encountered.

Every path gets a trailing `/` appended if it lacks one. A filename beginning with `.`, `/`, `\`, or containing `:` bypasses the include paths entirely:
```c
  if (*filename=='.' || *filename=='/' || *filename=='\\' ||
      strchr(filename,':')!=NULL) {
    /* file name is absolute, then don't use any include paths */
    if (f = fopen(filename,mode)) { add_depend(filename); return f; }
  }
```
There is **no** "relative to the including file" search — only the *main* input file's directory is registered, once. Failure → `general_error(12)` = `error 13: could not open <%s> for input`, FATAL.

---

## Gotchas checklist for a byte-exact reimplementation

1. `BOOLEAN(x)` = `-(x)`: comparisons produce **-1**, but unary `!` produces **1**.
2. Operator precedence is `<< >>` > `&` > `^ ~` > `| !` > `* / % //` > `+ -` > relational > equality > `&&` > `||`.
3. `esc_sequences` is **off** by default — a backslash in a string is a literal backslash.
4. Without `-spaces`, the first blank inside an operand truncates the line (destructively, in the shared line buffer).
5. Data is **not** auto-aligned by default; only instructions (`INST_ALIGN 2`) and explicit `cnop`/`even`/`align`.
6. `cnop 0,N` with `N>3` in a `c`-attributed section pads with `4E71`, via the `space==0` branch of `fwpcalign`.
7. `equ` and `=` are the same and are non-redefinable; only `set` allows redefinition.
8. Local label mangling keeps the leading `.` but drops the trailing `$`; scope is only reset by a new global *label* (never by `section`/`equ`/`include`).
9. `taddr` is `int32_t` throughout `eval_expr`; only literals wider than 32 bits are promoted to `thuge` and they are truncated (with warning 21) on conversion back.
10. `eval_expr` writes `*result` even when it returns 0; a label yields its *current* pc.
11. `-Fbin` writes `ds`/`dcb` space in full, fills inter-section gaps with zeros, sorts sections by `org`, and errors on any overlap — which means two non-empty relocatable sections always fail.
12. `RORG`/`RORGEND` atoms have zero size and are ignored by the binary writer, so `org` inside a `section` shifts symbol values but not file offsets.
13. `incbin` in 1.7h takes **only** a filename.
14. There is no `assert` directive and no `LIST` atom type in this version.
15. Default `max_errors` is 5, and a non-zero error count deletes the output file in `leave()`.