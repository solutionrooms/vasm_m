<!-- Generated 2026-09-10 by a research agent reading vasm-1.7h/cpus/m68k; verified against the reference binary. -->

# vasm 1.7h m68k backend — implementation spec (plain 68000, Motorola syntax, `-Fbin`)

Files: `/Users/jonscott/Projects/vasm_m/vasm-1.7h/cpus/m68k/{cpu.h,cpu.c,opcodes.h,operands.h,specregs.h,cpu_models.h,cpu_errors.h}`, plus core `/Users/jonscott/Projects/vasm_m/vasm-1.7h/{vasm.c,atom.c,expr.c,supp.c,reloc.c,error.c,output_bin.c}` and `/Users/jonscott/Projects/vasm_m/vasm-1.7h/syntax/mot/syntax.c`.

There is a prebuilt reference binary at `/Users/jonscott/Projects/vasm_m/vasm-1.7h/vasmm68k_mot` (arm64 Mach-O) — use it as your differential oracle.

---

## 0. Global control flow

```
mot syntax parse loop (syntax.c:1780-1883)
  -> parse_cpu_special()            cpu.c:5222   (near/far/basereg/machine/opt/fpu/...)
  -> handle_directive()             mot
  -> parse_instruction()            cpu.c:4880   (splits "move.w" into name + qualifier)
  -> skip_operand() per operand     mot syntax.c:165
  -> new_inst()                     atom.c:10    (mnemonic selection; calls parse_operand)
  -> new_inst_atom()                atom.c:460   (align = INST_ALIGN = 2)

resolve()      vasm.c:270   final_pass=0, loop passes until sizes stable
  -> atom_size() -> instruction_size()   cpu.c:3619   (optimizes a *copy*, final=0)

assemble()     vasm.c:280   final_pass=1, one pass
  -> eval_instruction()             cpu.c:4244   (optimizes the *real* ip, final=1, emits bytes)
  -> eval_data()                    cpu.c:4538

output_bin write_output()           output_bin.c:24
```

`taddr` is `int32_t`, `utaddr` is `uint32_t` (`cpu.h:20-21`). `bytespertaddr=4`, `bitsperbyte=8`, `BIGENDIAN 1` (`cpu.c:29-30`, `cpu.h:6`). `MAX_OPERANDS 6`, `MAX_QUALIFIERS 1` (`cpu.h:12,15`).

---

## 1. The `mnemonics[]` table

### 1.1 Struct definitions (exact)

`vasm.h:130-137`:
```c
/* mnemonic description */
typedef struct mnemonic {
  char *name;
#if MAX_OPERANDS!=0
  int operand_type[MAX_OPERANDS];
#endif
  mnemonic_extension ext;
} mnemonic;
```

`cpu.h:258-264`:
```c
/* additional mnemonic data */
typedef struct {
  uint16_t place[MAX_OPERANDS];
  uint16_t opcode[2];
  uint16_t size;
  uint32_t available;
} mnemonic_extension;
```

So a row in `opcodes.h` is:
```
  "add",   {DA,D_},   {{SEA,RHI},   {0xd000,0},  1|CFBWL|S_STD,  m68000up|mcf},
   name   operand_type   place         opcode[]      size            available
```
`mnemonics[]` is instantiated at `cpu.c:16-19` by `#include "opcodes.h"`.

The `size` field packs four things:

| bits | meaning | macros |
|---|---|---|
| 0-1 | number of opcode words emitted (1, 2 or 3) | `S_OPCODE_SIZE(n) (n&3)` (`cpu.h:291`) |
| 2-6 | *size-insertion mode* | `S_SIZEMODE(n) (n&0x7c)` (`cpu.h:292`) |
| 7 | `S_CFCHECK` — ColdFire-only restriction | `cpu.h:277` |
| 8-14 | `SIZE_*` mask of permitted extensions | `SIZE_MASK 0x7f00` |
| 15 | `SIZE_UNAMBIG` — set by `init_cpu()` when exactly one size bit is set | `cpu.h:276`, set at `cpu.c:4673-4676` |

`cpu.h:266-311` (verbatim):
```c
/* size qualifiers, lowest two bits specify opcode size in words! */
#define SIZE_UNSIZED 0
#define SIZE_BYTE 0x100
#define SIZE_WORD 0x200
#define SIZE_LONG 0x400
#define SIZE_SINGLE 0x800
#define SIZE_DOUBLE 0x1000
#define SIZE_EXTENDED 0x2000
#define SIZE_PACKED 0x4000
#define SIZE_MASK 0x7f00
#define SIZE_UNAMBIG 0x8000 /* only a single size allowed for this mnemonic */
#define S_CFCHECK 0x80      /* SIZE_LONG only, when mcf (Coldfire) set */
#define S_NONE 4
#define S_STD S_NONE+4      /* 1st word, bits 6-7 */
#define S_STD1 S_STD+4      /* 1st word, bits 6-7, b=1,w=2,l=3  */
#define S_HI S_STD1+4       /* 1st word, bits 9-10 */
#define S_CAS S_HI+4        /* 1st word, bits 9-10, b=1,w=2,l=3 */
#define S_MOVE S_CAS+4      /* move instruction, 1st word bits 12-13 */
#define S_WL8 S_MOVE+4      /* w/l flag in 1st word bit 8 */
#define S_LW7 S_WL8+4       /* l/w flag in 1st word bit 7 */
#define S_WL6 S_LW7+4       /* w/l flag in 1st word bit 6 */
#define S_TRAP S_WL6+4      /* 1st word, bits 1-0, w=2, l=3 */
#define S_EXT S_TRAP+4      /* 2nd word, bits 6-7 */
#define S_FP S_EXT+4        /* 2nd word, bits 12-10 (l=0,s,x,p,w,d,b) */
#define S_MAC S_FP+4        /* w/l flag in 2nd word bit 11 */
...
/* short cuts */
#define UNS SIZE_UNSIZED
#define B SIZE_BYTE
#define W SIZE_WORD
#define L SIZE_LONG
#define Q SIZE_DOUBLE
#define SBW (SIZE_BYTE|SIZE_WORD|SIZE_SINGLE)  /* .s = .b for branches */
#define SBWL (SIZE_BYTE|SIZE_WORD|SIZE_LONG|SIZE_SINGLE)
#define BW (SIZE_BYTE|SIZE_WORD)
#define WL (SIZE_WORD|SIZE_LONG)
#define BWL (SIZE_BYTE|SIZE_WORD|SIZE_LONG)
#define CFWL (SIZE_WORD|SIZE_LONG|S_CFCHECK)
#define CFBWL (SIZE_BYTE|SIZE_WORD|SIZE_LONG|S_CFCHECK)
```
Numeric values for the 68000-relevant size modes: `S_NONE=4, S_STD=8, S_STD1=12, S_HI=16, S_CAS=20, S_MOVE=24, S_WL8=28, S_LW7=32, S_WL6=36, S_TRAP=40, S_EXT=44, S_FP=48, S_MAC=52`.

**For a 68000-only port**, `S_CFCHECK` never fires (`cpu_type & mcf` is always 0), so `CFWL≡WL` and `CFBWL≡BWL`. Only `S_NONE, S_STD, S_STD1, S_HI, S_LW7, S_WL6, S_WL8, S_MOVE` occur in 68000-eligible rows (`S_CAS/S_EXT/S_TRAP/S_FP/S_MAC` are 020+/CPU32/FPU/MAC only).

### 1.2 Availability flags (`cpu.h:351-387`, verbatim)

```c
/* cpu types for availability check - warning: order is important */
#define CPUMASK  0x000fffff
#define	m68000   0x00000001
#define	m68010   0x00000002
#define	m68020   0x00000004
#define	m68030   0x00000008
#define	m68040   0x00000010
#define m68060   0x00000020
#define	m68881   0x00000040
#define	m68882   m68881
#define	m68851   0x00000080
#define cpu32    0x00000100
#define mcfa     0x00000200
...
#define mgas     0x20000000 /* a GNU-as specific mnemonic */
#define malias   0x40000000 /* a bad alias which we should warn about */
#define mfpu     0x80000000 /* just to check if CP-ID needs to be inserted */
...
#define	m68010up  (m68010|cpu32|m68020up)
#define	m68000up  (m68000|m68010up)
```
Availability test is `mnemo->ext.available & cpu_type` (a bitwise-AND, not a `>=` comparison). With `cpu_type == m68000`, a row is eligible iff its `available` field has bit 0 set — i.e. iff it contains `m68000up` (or bare `m68000`).

- `mgas` rows: `init_cpu()` (`cpu.c:4648-4658`) **removes them from the hash table** unless `-gas`. Since they are still `m68000up`, they'd otherwise be reachable; without `-gas` they are unreachable. For a Motorola-only port, delete every `mgas` row (`mov`, `movm`, `jra`, `jbra`, `jbsr`, `jhi`…`jble`, `jbhi`…`jble`).
- `malias` rows: still assembled, but `eval_instruction` emits `cpu_error(33)` ("deprecated instruction alias", WARNING) at `cpu.c:4279-4280`. Only `movea {A_,AL}` (line 867) and `movea {DA,AD}` (line 868) carry it in the 68000 set.
- `mfpu`/`mcffpu` change the first opcode byte (`cpu.c:4283-4286`) — irrelevant on 68000.

The complete set of 68000-eligible rows is exactly the `grep`-able set of `opcodes.h` lines containing `m68000up` or `m68000`: lines 15–45, 48–103 (`.b/.w` branch rows only — the second `SBWL` row of each `b<cc>` is `m68020up|cpu32|mcfb|mcfc`), 119, 121, 123, 125, 127, 128, 130, 132, 134, 135, 137, 146–164, 165, 167, 171, 173, 177–190, 698, 768, 771, 772, 773–777, 780–787, 834–848, 866–868, 871, 874, 876, 877, 880, 882, 883–885, 893–902 (mgas), 915, 917, 920, 922, 927–943, 978, 1111–1127, 1129, 1131, 1132, 1134–1172, 1174–1187, 1198, 1199, 1236, 1239, 1244.

### 1.3 Operand-type codes

`operands.h:56-62` declares the enum; `operands.h:64-280` the `optypes[]` table. Struct (`cpu.h:237-255`):
```c
/* operand types */
struct optype {
  uint16_t modes;         /* addressing modes allowed (0-15, see above) */
  uint16_t flags;
  signed char first;
  signed char last;
};

/* flags */
#define OTF_NOSIZE    1   /* this addr. mode requires no additional bytes */
#define OTF_BRANCH    2   /* branch instruction */
#define OTF_DATA      4   /* data definition */
#define OTF_FLTIMM    8   /* base10 immediate values are floating point */
#define OTF_QUADIMM  16   /* immediate values are 64 bits */
#define OTF_SPECREG  32   /* check for special registers during parse */
#define OTF_SRRANGE  64   /* check range between first/last only */
#define OTF_REGLIST 128   /* register list required, even when single reg. */
#define OTF_CHKVAL  256   /* compare op. value against first/last */
#define OTF_CHKREG  512   /* compare op. register against first/last */
```
`modes` is a bitmask over `AM_*` indices (`cpu.h:219-234`), which index `addrmodes[]` (`operands.h:1-18`):

| bit | `AM_` | `(mode,reg)` |
|---|---|---|
|0|`AM_Dn`|`MODE_Dn,-1`|
|1|`AM_An`|`MODE_An,-1`|
|2|`AM_AnIndir`|`MODE_AnIndir,-1`|
|3|`AM_AnPostInc`|`MODE_AnPostInc,-1`|
|4|`AM_AnPreDec`|`MODE_AnPreDec,-1`|
|5|`AM_An16Disp`|`MODE_An16Disp,-1`|
|6|`AM_An8Format`|`MODE_An8Format,-1`|
|7|`AM_AbsShort`|`MODE_Extended,REG_AbsShort`|
|8|`AM_AbsLong`|`MODE_Extended,REG_AbsLong`|
|9|`AM_PC16Disp`|`MODE_Extended,REG_PC16Disp`|
|10|`AM_PC8Format`|`MODE_Extended,REG_PC8Format`|
|11|`AM_Immediate`|`MODE_Extended,REG_Immediate`|
|12|`AM_RnList`|`MODE_Extended,REG_RnList`|
|13|`AM_FPnList`|`MODE_Extended,REG_FPnList`|
|14|`AM_FPn`|`MODE_FPn,-1`|
|15|`AM_SpecReg`|`MODE_SpecReg,-1`|

The `_(a,b,…,p)` macro (`operands.h:51-54`) builds the mask from 16 positional bits (bit 0 = `AM_Dn`).

**Every operand type used by 68000-eligible rows:**

| code | modes (AM bits) | flags | first/last | meaning |
|---|---|---|---|---|
| `OP_D8/D16/D32/D64` | 11 | `OTF_DATA` (+`OTF_QUADIMM` for D64) | | data definition of 8/16/32/64 bits |
| `OP_F32/F64/F96` | 11 | `OTF_DATA|OTF_FLTIMM` | | float data |
| `D_` | 0 | – | | data register `Dn` |
| `A_` | 1 | – | | address register `An` |
| `AI` | 2 | – | | `(An)` |
| `R_` | 0,1 | – | | any `Dn` or `An` |
| `PA` | 4 | – | | `-(An)` |
| `AP` | 3 | – | | `(An)+` |
| `DP` | 2,5 | – | | `d(An)` for `movep` (`(An)` auto-translated to `0(An)`) |
| `IM` | 11 | – | | immediate |
| `QI` | 11 | `OTF_NOSIZE` | | "quick" immediate — contributes **0** extension bytes |
| `IR` | 11 | `OTF_NOSIZE` | | immediate register-list bitmask (`movem #mask,…`) |
| `BR` | 8 | `OTF_BRANCH` | | branch destination (parsed as abs-long; size from `branch_size()`) |
| `VA` | 8 | `OTF_NOSIZE` | | absolute value (LINE-A/LINE-F 12-bit) |
| `RL` | 12 | `OTF_REGLIST` | | `Dn/An` register list |
| `AY` | 0-11 | – | | all modes 0-6, 7.0-4 |
| `AM` | 2-8 | – | | alterable memory 2-6, 7.0-1 |
| `MA` | 2-11 | – | | memory 2-6, 7.0-4 |
| `MI` | 2-10 | – | | memory 2-6, 7.0-3 (no immediate) |
| `AL` | 0-8 | – | | alterable 0-6, 7.0-1 |
| `DA` | 0,2-11 | – | | data 0, 2-6, 7.0-4 |
| `DN` | 0,2-10 | – | | data, no immediate |
| `CT` | 2,5,6,7,8,9,10 | – | | control 2, 5-6, 7.0-3 |
| `AC` | 2,5,6,7,8 | – | | alterable control 2, 5-6, 7.0-1 |
| `AD` | 0,2-8 | – | | alterable data 0, 2-6, 7.0-1 |
| `MS` | 2,4,5,6,7,8 | – | | `movem` save operands 2, 4-6, 7.0-1 |
| `MR` | 2,3,5,6,7,8,9,10 | – | | `movem` restore operands 2-3, 5-6, 7.0-3 |
| `_CCR` | 15 | `OTF_SPECREG|OTF_CHKREG` | `REG_CCR,REG_CCR` | literal `CCR` |
| `_SR` | 15 | `OTF_SPECREG|OTF_CHKREG` | `REG_SR,REG_SR` | literal `SR` |
| `_USP` | 15 | `OTF_SPECREG|OTF_CHKREG` | `REG_USP,REG_USP` | literal `USP` |

(020+/CF/FPU-only types you can drop: `RM,DD,CS,F_,FF,FR,FPIAR,FL,FS,CF,MAQ,CFAM,CM,CFDA,CFAD,BD,BS,AK,CFMM,CFMN,MAQ,SH`, and the `_CACHES/_ACC/_MACSR/_MASK/_CTRL/_ACCX/_AEXT/_VAL/_FC/_RP_*/_TC/_AC/_M1_B/_BAC/_BAD/_PSR/_PCSR/_TT` special-reg types.)

`FL_CheckMask 0xf800` (`cpu.h:127`) — the operand-`flags` bits compared for equality against the optype's `flags` field during the final match loop (`cpu.c:1687`): `FL_MAC|FL_Bitfield|FL_DoubleReg|FL_KFactor|FL_FPSpec`. All zero for 68000 types, so on 68000 the check reduces to "operand carries none of those flags".

### 1.4 The `place[]` field

`place[i]` indexes `insert_info[]` (`operands.h:369-546`), entries of `struct oper_insert` (`cpu.h:314-342`):
```c
/* operand insertion info */
struct oper_insert {
  unsigned char mode;         /* insert mode (see below) */
  unsigned char size;         /* number of bits to insert */
  unsigned char pos;          /* bit position for inserted value in stream */
  unsigned char flags;
  void (*insert)(unsigned char *,struct oper_insert *,operand *);
};

/* insert modes */
#define M_nop         0       /* do nothing for this operand */
#define M_noea        1       /* don't store ea, only extension words */
#define M_ea          2       /* insert mode/reg in lowest 6 bits */
#define M_high_ea     3       /* insert reg/mode in bits 11-6 (MOVE) */
#define M_bfea        4       /* insert std. ea and bitfield offset/width */
#define M_kfea        5       /* insert std. ea and k-factor/dest.format */
#define M_func        6       /* use insert() function */
#define M_branch      7       /* extval0 contains branch label */
#define M_val0        8       /* extval0 at specified position */
#define M_reg         9       /* insert reg at specified position */
/* flags */
#define IIF_MASK      1       /* value 2^size is represented by a 0 (M_val0)
                                 recognize MASK-flag for MAC instr. (M_ea) */
#define IIF_BCC       2       /* Bcc branch, opcode is modified */
#define IIF_REVERSE   4       /* store bits in reverse order (M_val0) */
#define IIF_NOMODE    8       /* don't store ea mode specifier in opcode */
#define IIF_SIGNED   16       /* value is signed (M_val0) */
#define IIF_3Q       64       /* MOV3Q: -1 is written as 0 (M_val0) */
#define IIF_ABSVAL  128       /* make sure first expr. is absolute (M_func) */
```
`pos` is a **bit offset from the start of the instruction**, big-endian (bit 0 = MSB of the first opcode word). So `pos=4,size=3` → bits 11-9 of word 0; `pos=16,size=16` → word 1.

68000-relevant `place` codes (`operands.h:369-546`):

| code | mode,size,pos,flags | effect |
|---|---|---|
| `NOP` | `M_nop` | nothing (special regs `CCR/SR/USP`) |
| `NEA` | `M_noea` | emit extension words only, don't OR an EA into the opcode |
| `SEA` | `M_ea` | standard EA in bits 5-0 + extension words |
| `MEA` | `M_high_ea` | MOVE destination EA in bits 11-6 |
| `REA` | `M_ea,…,IIF_NOMODE` | only the *register* part (bits 2-0) — `movep` |
| `BRA` | `M_branch,…,IIF_BCC` | Bcc/BRA/BSR displacement, opcode low byte patched |
| `DBR` | `M_branch,0` | DBcc word displacement (never patches low byte) |
| `RHI` | `M_reg,3,4` | register in bits 11-9 |
| `RLO` | `M_reg,3,13` | register in bits 2-0 |
| `DL8` | `M_val0,8,8,IIF_SIGNED` | 8-bit signed value in low byte (`moveq`) |
| `DL4` | `M_val0,4,12` | 4-bit value bits 3-0 (`trap`) |
| `D3Q` | `M_val0,3,4,IIF_MASK` | addq/subq 3-bit quick, bits 11-9, `8↔0` |
| `DL3` | `M_val0,3,13` | 3-bit bits 2-0 (`bkpt`) |
| `D16` | `M_val0,16,16` | 16-bit value in word 1 (`stop`, movem mask, `link` … ) |
| `D2R` | `M_val0,16,16,IIF_REVERSE` | 16-bit **bit-reversed** in word 1 (movem predecrement) |
| `EL8` | `M_val0,8,24` | 8-bit in low byte of word 1 (`btst/bchg/bclr/bset #n`) |
| `LIN` | `M_val0,12,4` | 12-bit in bits 11-0 (`linea`/`linef`) |

### 1.5 Table ordering rules (from `opcodes.h:1-14`, verbatim)

```
/* Rules for adding mnemonics with the same name but with different
   operand types or cpu-requirements to this table:
   1. When operand types are a subset of another instruction, place it
      *before* that mnemonic.
   2. When operand types match, but cpu-requirements are higher or completely
      different, place it *after* that mnemonic (also important for next
      rule).
   3. Mnemonics with different operation sizes (opcode extensions), but same
      name and operand types, should be kept together. vasm will scan through
      them to find the correct size, and stop on the first different name
      or operand type.
   4. Mnemonics without an operand ({0}) must be the last of those which have
      the same name.
*/
```
All rows with the same name must be **contiguous** — the hash table (`vasm.c:513-522`) maps each distinct name to the index of its *first* row and then relies on contiguity.

Two additional ordering contracts you must preserve if you prune rows:
- `opcodes.h:46-47`: *"Two conditional branches must always be followed by two branches with the same, but negated, condition!"* — `optimize_instruction` does `ip->code += (oc&0x0100) ? -2 : 2;` (`cpu.c:3394`, `cpu.c:3466`) to negate a branch condition.
- `opcodes.h:872,878`: *"two src-RL must be followed by two dest-RL with swapped operands"* — the movem swap does `ip->code += 2` (`cpu.c:2378`). The intervening ColdFire rows (873, 875, 879, 881) exist purely to make that `+2` land correctly. If you delete the CF rows, replace `+2` with an explicit index.

### 1.6 Mnemonic selection — four stages

**Stage A — `new_inst()` (`atom.c:10-126`).** `find_namelen_nc(mnemohash, …)` (always case-insensitive, `symtab.c:143`). Then scan forward:
```c
do {
  inst_found = 1;
  ...
  for (j=0; j<MAX_OPERANDS; j++)
    if (mnemonics[i].operand_type[j] == 0) break;
  mnemo_opcnt = j;
  inst_found = 2;
  save_symbols();
  for (j=k=omitted=skipped=0; j<mnemo_opcnt; j++) {
      if (k >= op_cnt) break;
      rc = parse_operand(op[k],op_len[k],&ops[j],mnemonics[i].operand_type[j]);
      if (rc == PO_CORRUPT) { ...return 0; }
      if (rc == PO_NOMATCH) break;
      k++;
  }
  if (j<mnemo_opcnt || k<op_cnt) { i++; restore_symbols(); continue; }  /* no match */
  ...
  new->code = i; return new;
} while (i<mnemonic_cnt && !strnicmp(mnemonics[i].name,inst,len)
         && mnemonics[i].name[len]==0);
```
First row whose operand count *and* every operand type match wins. **CPU availability is not consulted here** (`MNEMONIC_VALID` defaults to 1, `vasm.h:43-45`; m68k does not override it). `OPERAND_OPTIONAL` is also the default 0, so no operands are optional.

Failure diagnostics: `general_error(8)` "instruction not supported by cpu" if only stage-1 was reached, `general_error(0)` "illegal operand types" if operands were tried, `general_error(1)` if the name is unknown.

**Stage B — CPU-availability advance, `instruction_size()` (`cpu.c:3633-3642`):**
```c
  while (!(mnemo->ext.available & cpu_type)) {
    mnemonic *lastm = mnemo;
    mnemo++;
    if (strcmp(lastm->name,mnemo->name) || !optypes_subset(lastm,mnemo))
      cpu_error(0);  /* instruction not supported */
    realip->code++;
  }
```
e.g. `cmp.w a0,d0` matches row 122 (`mcfb|mcfc`) in stage A, then advances to row 123 (`m68000up|mcf`).

**Stage C — size-extension advance (`cpu.c:3697-3742`):**
```c
    uint16_t sz  = lc_ext_to_size(ext);
    if ((mnemo->ext.size&SIZE_UNAMBIG) && (mnemo->ext.available&cpu_type))
      uacode = realip->code;
    else uacode = -1;
    while (!((((extsize&S_CFCHECK) && (cpu_type&mcf)) ?
              (extsize & ~(SIZE_BYTE|SIZE_WORD)) : extsize) & sz)) {
      mnemo++;
      if (err = strcmp(mnemonics[realip->code].name,mnemo->name)) break;
      for (i=0; i<MAX_OPERANDS; i++)
        if (err = (mnemonics[realip->code].operand_type[i] != mnemo->operand_type[i])) break;
      if (err) break;
      realip->code++;
      extsize = mnemo->ext.size;
      if ((mnemo->ext.size&SIZE_UNAMBIG) && (mnemo->ext.available&cpu_type))
        uacode = realip->code;
    }
    if (err) {
      if (ign_unambig_ext && uacode>=0) { /* -guess-ext: use the unambiguous row */ }
      else cpu_error(34);  /* illegal opcode extension */
    }
    if (!(mnemo->ext.available & cpu_type)) cpu_error(0);
```
Requires **identical name and identical `operand_type[]` array** — stricter than stage B.

**Stage D — generalisation advance, `optimize_instruction()` (`cpu.c:2335-2352`):**
```c
  while (!strcmp(mnemo->name,mnemonics[ip->code+1].name) &&
         (mnemonics[ip->code+1].ext.available & cpu_type) != 0) {
    mnemonic *nextmn = &mnemonics[ip->code+1];
    uint16_t nextsize = nextmn->ext.size;
    if ((mnemo->ext.size&SIZE_MASK) != SIZE_UNSIZED ||
        (nextsize&SIZE_MASK) != SIZE_UNSIZED) {
      if ((nextsize&S_CFCHECK) && (cpu_type&mcf))
        nextsize &= ~(SIZE_BYTE|SIZE_WORD);
      if ((nextsize & lc_ext_to_size(ext)) == 0) break;
    }
    if (!optypes_subset(mnemo,nextmn)) break;
    ip->code++;
  }
```
`optypes_subset(mold,mnew)` (`cpu.c:2178-2208`) — true when for every slot `optypes[mnew].modes ⊇ optypes[mold].modes` and `FL_CheckMask` flags are equal and operand counts agree. (Note the `break` at line 2203 is outside the `if` — a latent bug that makes the `OTF_SPECREG` range check terminate the loop after slot 0; replicate it if you care about `move ccr,…`-class rows, though on 68000 it's benign.)

---

## 2. `parse_operand()` — addressing-mode recognition (`cpu.c:1199-1728`)

### 2.1 `operand` struct and encodings

`cpu.h:92-105`:
```c
/* type to store each operand */
typedef struct {
  signed char mode;
  signed char reg;
  uint16_t format;            /* used for (d8,An/PC,Rn) and ext.addr.modes */
  unsigned char bf_offset;    /* bitfield offset, k-factor or MAC Upper Word */
  unsigned char bf_width;     /* bitfield width or MAC-MASK '&' */
  int8_t basetype[2];         /* BASE_OK=normal, BASE=PCREL=pc-relative base */
  uint32_t flags;
  expr *value[2];             /* immediate, abs. or displacem. expression */
  /* filled during instruction_size(): */
  taddr extval[2];            /* evaluated expression from value[0/1] */
  symbol *base[2];            /* symbol base for value[0/1], NULL otherwise */
} operand;
```
`cpu.h:131-149`:
```c
/* addressing modes */
#define MODE_Dn           0
#define MODE_An           1
#define MODE_AnIndir      2
#define MODE_AnPostInc    3
#define MODE_AnPreDec     4
#define MODE_An16Disp     5
#define MODE_An8Format    6   /* uses format word */
#define MODE_Extended     7   /* reg determines addressing mode */
#define MODE_FPn          8   /* FPU register */
#define MODE_SpecReg      9   /* reg determines index into SpecRegs */
/* reg encodings for MODE_Extended: */
#define REG_AbsShort      0
#define REG_AbsLong       1
#define REG_PC16Disp      2
#define REG_PC8Format     3   /* uses format word */
#define REG_Immediate     4
#define REG_RnList        5   /* An/Dn register list in value[0] */
#define REG_FPnList       6   /* FPn register list in value[0] */
```
`MODE_*` 0-7 map 1:1 onto the hardware mode field; `MODE_Extended`'s `reg` 0-4 map 1:1 onto the hardware `reg` field for mode 7. `REG_RnList`/`REG_FPnList` are pseudo-values that never reach the encoder as an EA.

Operand flags (`cpu.h:107-129`, the 68000-relevant ones):
```c
#define FL_ExtVal0          1   /* extval[0] is set */
#define FL_ExtVal1          2   /* extval[1] is set */
#define FL_UsesFormat       4   /* operand uses format word */
#define FL_020up            8   /* 020+ addressing mode */
#define FL_noCPU32       0x10
#define FL_NoOptBase    0x100   /* never optimize base displacement */
#define FL_NoOptOuter   0x200   /* never optimize outer displacement */
#define FL_NoOpt        0x300   /* never optimize this whole operand */
#define FL_DoNotEval    0x400   /* do not evaluate, extval and base are ok */
#define FL_CheckMask    0xf800
#define FL_BaseReg    0x10000   /* BASEREG expression in exp.value[0] */
```
Format word bits (`cpu.h:151-171`) — for 68000 only the *brief* format matters: `FW_IndexAn 0x8000`, `FW_IndexReg(n) ((n)<<12)`, `FW_LongIndex 0x0800`, `FW_Scale(n) ((n)<<9)` (68000 must be 0), `FW_FullFormat 0x0100` (020+ only). The brief format word emitted is `(format>>8)&0xff` in the high byte and the 8-bit displacement in the low byte (`cpu.c:4037-4038`, `cpu.c:4134-4135`).

### 2.2 Register scanning

`getreg()` (`cpu.c:624-689`) returns a packed byte:
```
  Bit7 6     5     4     3         2    1    0
   ---------------------------------------------
  | 0 | extension (1-7) | An reg. | reg. number |
   ---------------------------------------------
```
Accepts: register symbols created by `EQUR` (`find_regsym`), `d0`-`d7`, `a0`-`a7` (either case), and `sp` (=`a7`, returns 15). With `-elfregs` a leading `%` is required. A `.x` suffix sets bits 4-6 (`getextcode`: b=1,w=2,l=3,s=4,d=5,x=6,p=7; or for non-index registers `getmacextcode`: l=`EXT_LOWER`=3, u=`EXT_UPPER`=2). `An_only` argument raises `cpu_error(4)` "address register required".

`getbasereg()` (`cpu.c:988-1072`) — for base/index positions; adds `pc`, `zd0..zd7`, `za0..za7`, `zpc` and `*1/*2/*4/*8`:
```
   Bits 0-4: 0-7 = d0-d7, 8-15 = a0-a7 (bit 3 = An), 16 = pc
   Bit  7    = Zero-flag (zdn,zan,zpc)
   Bits 8-10 = optional extension (1=.b, 2=.w ... 7=.p)
   Bits 12-13= scale factor (0=*1, 1=*2, 2=*4, 3=*8)
```
Bad scale → `cpu_error(10)`. Register macros: `REGAn 8`, `REGPC 16`, `REGZero 0x80`, `REGget(n) ((n)&7)`, `REGgetA(n) ((n)&15)`, `REGext(n) (((n)&0x700)>>8)`, `REGscale(n) (((n)&0x3000)>>12)`.

`scan_Rnlist()` (`cpu.c:692-750`) builds a 16-bit mask, bit `n` = `Dn`, bit `8+n` = `An`. Supports `/`, `-` ranges, and the `d0-7` shorthand (`cpu.c:705-708`). Reversed ranges are silently normalised; `Rn-Rn` and duplicate registers → `cpu_error(17)` (WARNING). A leading non-register → returns 0 (empty mask, no error); a non-register *after* the first → `cpu_error(2)` "invalid register list".

### 2.3 The parse itself

```c
int parse_operand(char *p,int len,operand *op,int required)
{
  uint16_t reqmode = optypes[required].modes;
  uint16_t reqflags = optypes[required].flags;
  ...
  op->mode = op->reg = -1;  op->flags = 0;  op->format = 0;
  op->value[0] = op->value[1] = NULL;
  p = skip(p);
```
Dispatch order:

1. **`OTF_DATA`** (`1223-1229`) → `MODE_Extended/REG_Immediate`, expression parsed as float if `OTF_FLTIMM`, as huge if `OTF_QUADIMM`.
2. **`#`** (or `&` with `-sgs`) (`1230-1237`) → `MODE_Extended/REG_Immediate`. Float only if `OTF_FLTIMM && is_float_ext()` (i.e. the instruction extension is `s/d/x/p`, `cpu.c:580-590`).
3. **`get_any_register()`** (`cpu.c:865-985`) — tries `Dn/An` (→ `MODE_Dn`/`MODE_An`, or `MODE_Extended/REG_RnList` if the next char is `-`/`/` **or** `OTF_REGLIST` is set), then `FPn`, then a `SpecRegs[]` name (case-insensitive, exact length, and `SpecRegs[i].available & cpu_type` must hold) → `MODE_SpecReg`, `reg` = index into `SpecRegs[]`, `value[0]` = `number_expr(code)`. On 68000 only `CCR`, `SR`, `USP` are available (`specregs.h:1,2,82`); `USP`'s `code` 0x800 is unused (place is `NOP`/`RLO`).
   After that, `Dm:Dn` and `FPm:FPn` handling (020+/FPU only).
4. **`-(An)`** (`1278-1293`) → `MODE_AnPreDec`. Missing `)` → `cpu_error(3)`.
5. **Bare expression** (`1295-1300`) when the text does not start with `(` → `base_disp_and_ext()`:
```c
static int base_disp_and_ext(operand *op,char **p)
{
  if (op->value[0] = parse_expr(p)) {
    int disp_size;
    if (disp_size = read_extension(p,0))
      op->flags |= FL_NoOptBase;  /* do not optimize, when size is given */
    return disp_size;
  }
  return -1;
}
```
   **This is where `.w`/`.l` on an absolute or a displacement both selects the size *and* disables optimisation of that operand.**
6. **`(` …** (`1302-1606`) — the big block. Key path for 68000:
   - `getbasereg()` at the head. If it fails and no value has been read, re-parse as an expression; if the next char is `)` the whole thing was only the *first term* of a larger expression → rewind to `start_term` and `goto parse_expression` (this is what makes `(label2-label1)-2(pc,d1.w)` work). If the next char is `,` → `(disp,Rn)` form.
   - `(Rn,bd)` and `(An,bd)` are accepted as `(bd,Rn)` with `cpu_error(6)` "displacement at bad position" (WARNING) unless `-devpac` (`1397-1408`, `1466-1475`).
   - Index register validation (`1487-1497`): non-ColdFire allows only `.w`/`.l` or none; anything else → `cpu_error(5)` and the extension is cleared. `PC` as index → `cpu_error(16)` + `PO_CORRUPT`.
   - **Default displacement sizes** (`1500-1502`):
     ```c
     if (!disp_size)
       disp_size = idx<0 ? EXT_WORD : EXT_BYTE;
     ```
   - The 68000 mode decision (`1504-1571`):
     ```c
     if (!mem_indir && !REGisZero(reg) && op->value[1]==NULL &&
         ((idx<0 && disp_size==EXT_WORD) ||
          (idx>=0 && disp_size==EXT_BYTE && !REGisZero(idx)))) {
       if (idx < 0) {
         if (op->value[0]) {
           if (REGisPC(reg)) { op->mode = MODE_Extended; op->reg = REG_PC16Disp; }   /* (d16,PC) */
           else { op->mode = MODE_An16Disp; op->reg = REGget(reg); check_basereg(op); }
         }
         else {
           if (*p == '+') { op->mode = MODE_AnPostInc; ... p++; }   /* (An)+ */
           else { op->mode = MODE_AnIndir; ... }                     /* (An)  */
         }
       }
       else {
         if (!op->value[0]) op->value[0] = number_expr(0);   /* indexed always needs a d8 */
         if (REGisPC(reg)) { op->mode = MODE_Extended; op->reg = REG_PC8Format; }
         else { op->mode = MODE_An8Format; op->reg = REGget(reg); check_basereg(op); }
         set_index(op,idx);
       }
     }
     else { /* full format word: sets FL_020up — write_ea_ext will reject it on 68000 */ }
     ```
   - `set_index()` (`cpu.c:1075-1099`) ORs `FW_IndexAn | FW_IndexReg(i) | (REGext==EXT_LONG ? FW_LongIndex : 0) | FW_Scale(s)` and sets `FL_UsesFormat`; any scale ≠ 1 sets `FL_020up` on non-ColdFire.
7. **Absolute fallback** (`1608-1625`): value but no register →
```c
        op->mode = MODE_Extended;
        if (disp_size == EXT_WORD) op->reg = REG_AbsShort;
        else {
          if (reqflags & OTF_REGLIST) op->reg = (required==RL) ? REG_RnList : REG_FPnList;
          else op->reg = REG_AbsLong;
          if (disp_size!=0 && disp_size!=EXT_LONG) cpu_error(5);
        }
```
   So **no extension → abs.long** (later possibly shrunk by `opt_abs`), `.w` → abs.short (and `FL_NoOptBase` prevents any change), `.l` → abs.long + `FL_NoOptBase`. `.b`/`.s`/etc. → `cpu_error(5)` "bad size extension".
   Note the `OTF_REGLIST` case: a `movem` operand that is a plain symbol/number is treated as a register-list mask, not an address. `optimize_instruction` later un-does this if it turns out the other operand was the mask (`cpu.c:2372-2380`).
8. **Final match loop** (`1682-1727`):
```c
  for (i=0; i<16; i++) {
    if (reqmode & (1<<i)) {
      if ((op->flags&FL_CheckMask)==(reqflags&FL_CheckMask) &&
          addrmodes[i].mode==op->mode &&
          (addrmodes[i].reg<0 || addrmodes[i].reg==op->reg)) {
        if (reqflags & OTF_CHKREG) {
          if (op->reg < optypes[required].first || op->reg > optypes[required].last)
            return PO_NOMATCH;
        }
        if (required == DP) {
          op->flags |= FL_NoOpt;
          if (op->mode == MODE_AnIndir) {
            op->mode = MODE_An16Disp;  op->value[0] = number_expr(0);
            cpu_error(48,(int)op->reg,(int)op->reg);
          }
        }
        p = skip(p);
        if (*p=='\0' || p>=(start+len)) return PO_MATCH;
      }
    }
  }
  return PO_NOMATCH;
```
Note the trailing-garbage check: unless the operand text is fully consumed, it's `PO_NOMATCH`.

`check_basereg()`/`fix_basereg()` (`cpu.c:1125-1163`) implement the `BASEREG` directive: for `d(An)` with `An` in 0..6 and an active `baseexp[An]`, the displacement is rewritten as `expr - baseexp[An]` and `FL_BaseReg` is set.

### 2.4 `-spaces`, `-opt-*`, `-no-opt`, and the `opt_*` globals

**`-spaces` is a *syntax-module* option**, `syntax/mot/syntax.c:2287-2290` → `allow_spaces = 1`. It changes three things: `eol()` skips whitespace before checking EOL (`syntax.c:107-118`), `exp_skip()` does *not* terminate the operand at the first space (`syntax.c:147-162`), and the operand loop skips whitespace around commas (`syntax.c:1844-1850`). Without it, the first whitespace character inside an operand truncates the operand (`*s = '\0'`). It is implied by `-phxass` (`syntax.c:2278-2286`). It has no effect on encoding, only on what text reaches `parse_operand`.

**All `opt_*` globals and their defaults** (`cpu.c:43-72`, verbatim):
```c
static unsigned char opt_gen = 1;     /* generic optimizations (not Devpac) */
static unsigned char opt_movem = 0;   /* MOVEM Rn -> MOVE Rn */
static unsigned char opt_pea = 0;     /* MOVE.L #x,-(sp) -> PEA x */
static unsigned char opt_clr = 0;     /* MOVE #0,<ea> -> CLR <ea> */
static unsigned char opt_st = 0;      /* MOVE.B #-1,<ea> -> ST <ea> */
static unsigned char opt_lsl = 0;     /* LSL #1,Dn -> ADD Dn,Dn */
static unsigned char opt_mul = 0;     /* MULU/MULS #n,Dn -> LSL/ASL #n,Dn */
static unsigned char opt_div = 0;     /* DIVU/DIVS.L #n,Dn -> LSR/ASR #n,Dn */
static unsigned char opt_fconst = 1;  /* Fxxx.D #m,FPn -> Fxxx.S #m,FPn */
static unsigned char opt_brajmp = 0;  /* branch to different sect. into jump */
static unsigned char opt_pc = 1;      /* <label> -> (<label>,PC) */
static unsigned char opt_bra = 1;     /* B<cc>.L -> B<cc>.W -> B<cc>.B */
static unsigned char opt_allbra = 0;  /* also optimizes sized branches */
static unsigned char opt_disp = 1;    /* (0,An) -> (An), etc. */
static unsigned char opt_abs = 1;     /* optimize absolute addreses to 16bit */
static unsigned char opt_moveq = 1;   /* MOVE.L #x,Dn -> MOVEQ #x,Dn */
static unsigned char opt_quick = 1;   /* ADD/SUB #x,Rn -> ADDQ/SUBQ #x,Rn */
static unsigned char opt_branop = 1;  /* BRA.B *+2 -> NOP */
static unsigned char opt_bdisp = 1;   /* base displacement optimization */
static unsigned char opt_odisp = 1;   /* outer displacement optimization */
static unsigned char opt_lea = 1;     /* ADD/SUB #x,An -> LEA (x,An),An */
static unsigned char opt_lquick = 1;  /* LEA (x,An),An -> ADDQ/SUBQ #x,An */
static unsigned char opt_immaddr = 1; /* <op>.L #x,An -> <op>.W #x,An */
static unsigned char opt_speed = 0;   /* optimize for speed, not for size */
static unsigned char opt_sc = 0;      /* external JMP/JSR are 16-bit PC-rel. */
static unsigned char no_opt = 0;      /* don't optimize at all! */
static unsigned char warn_opts = 0;   /* warn on optimizations/translations */
static unsigned char convert_brackets = 0;
static unsigned char typechk = 1;     /* check value types and ranges */
static unsigned char ign_unambig_ext = 0;
```
Plus `cpu_type = m68000` (`cpu.c:34`), `sdreg = -1` (`cpu.c:36`), `fpu_id = 1`, `gas=sgs=no_fpu=elfregs=phxass_compat=devpac_compat=0`.

**Which matter on a plain 68000 with no options:** `opt_gen`, `opt_pc`, `opt_bra`, `opt_disp`, `opt_abs`, `opt_moveq`, `opt_quick`, `opt_branop`, `opt_lea`, `opt_lquick`, `opt_immaddr`. `opt_bdisp`/`opt_odisp` only touch 020+ full-format words (unreachable on 68000). `opt_fconst` is FPU-only. `opt_clr`, `opt_st`, `opt_pea`, `opt_movem`, `opt_lsl`, `opt_mul`, `opt_div`, `opt_speed`, `opt_brajmp`, `opt_allbra`, `opt_sc` are all off.

**Setting them:**
- Command line, `cpu_args()` (`cpu.c:4772-4877`): `-opt-movem`, `-opt-pea`, `-opt-clr`, `-opt-st`, `-opt-lsl`, `-opt-mul`, `-opt-div`, `-opt-fconst`, `-opt-brajmp`, `-opt-allbra`, `-opt-speed` — each does `opt_x = !no_opt` (so order matters: `-no-opt -opt-clr` leaves `opt_clr` at 0). `-no-opt` calls `clear_all_opts()` (`cpu.c:4762-4769`, zeroes **all** `opt_*` including `opt_gen`) and sets `no_opt = 1`. `-sc` sets `opt_sc`. `-showcrit`→`warn_opts=1`, `-showopt`→`warn_opts=2`. `-rangewarnings` → `modify_cpu_err(WARNING,25,29,32,36,0)` (`cpu.c:4838`). `-guess-ext`→`ign_unambig_ext`. `-conv-brackets`, `-elfregs`, `-sgs`, `-regsymredef`, `-sdreg=n` (2..6), `-mXXX`, `-no-fpu`.
- Source directives: `OPT` in Devpac form (`devpac_option`, `cpu.c:5010-5219`) — `O1..O12` map to `{OPTBRA,OPTDISP,OPTABS,OPTMOVEQ,OPTQUICK,NOP,OPTBRANOP,OPTBDISP,OPTODISP,OPTLEA,OPTLQUICK,OPTIMMADDR}` (`cpu.c:5013-5017`); `Oc/Od/Of/Og/Oj/Ol/Om/Op/Os/Ot/Ow/Ox` are vasm extensions; bare `O+/O-` toggles the whole safe set. `OPT`/`OPTC` in PhxAss form under `-phxass` (`cpu.c:4929-5007`).
- Every change goes through `add_cpu_opt()` (`cpu.c:367-386`) which appends an `OPTS` atom, so option changes are **positional within a section** and are re-applied by `cpu_opts()` during both `resolve_section` (`vasm.c:199-204`) and `assemble` (`vasm.c:379-382`). `cpu_opts_init()` (`cpu.c:423-434`) emits the initial atom set at the head of each section.
- Note `cpu_opts()` line 306: `if (cmd>OCMD_NOOPT && cmd<OCMD_OPTWARN && arg!=0) no_opt = 0;` — enabling *any* individual optimisation clears `no_opt`.

**Crucial subtlety:** `no_opt` is checked at `cpu.c:2415`, **after** the operand-level optimiser has already run (`cpu.c:2367-2369`). So `no_opt` alone does not disable `optimize_oper`. `-no-opt` works only because it also zeroes every `opt_*`. Verified: with `-no-opt`, `move.w $1000,d0` stays `3039 00001000`; without it, `3038 1000`.

---

## 3. Sizing, passes, and instruction-level optimisation

### 3.1 The resolver

`vasm.c:24-25`:
```c
#define MAXPASSES 1000
#define FASTOPTPHASE 50
```
`resolve_section()` (`vasm.c:174-268`):
- Loops `do { … } while(errors==0 && !done)`, `done` set to 1 at the top of each pass and cleared whenever a label moves or an atom's size changes.
- `extrapass = pass<=fastphase`. For `pass > fastphase` ("safe mode"), **once `done` has been cleared in this pass**, every further `INSTRUCTION` atom is skipped (`sec->pc += p->lastsize; continue;`) — i.e. at most one instruction changes size per pass.
- `if(p->changes>MAXSIZECHANGES)` (`MAXSIZECHANGES 5`, `atom.h:110`) → `sec->flags |= RESOLVE_WARN` around that atom's `atom_size()` call only, then cleared. `p->changes` is only incremented when `pass > fastphase`.
- `if(extrapass) fastphase++` — the fast phase is extended whenever no atom grew.

`assemble()` (`vasm.c:280-...`) runs once with `final_pass=1`; it sets `RESOLVE_WARN` for atoms with `changes>MAXSIZECHANGES` before `eval_instruction()` and clears it after each atom (`vasm.c:305-306`, `413`).

### 3.2 `instruction_size()` (`cpu.c:3619-3786`)

Order of operations:
1. Stage-B availability advance (§1.6).
2. `if (opt_allbra && ign_unambig_ext)` strip the size extension from any instruction with a `BR` operand (`3646-3656`) — off by default.
3. **Default extension assignment** when `ext=='\0'` (`3658-3686`):
```c
    realip->ext.un.real.flags |= IFL_UNSIZED;
    if ((extsize & SIZE_MASK) != 0) {
      if ((extsize & S_CFCHECK) && (cpu_type & mcf)) extsize &= ~(SIZE_BYTE|SIZE_WORD);
      if ((extsize & SIZE_LONG) && (cpu_type & mcf)) realip->qualifiers[0] = l_str;
      else if (extsize & SIZE_WORD)     realip->qualifiers[0] = w_str;
      else if (extsize & SIZE_BYTE)     realip->qualifiers[0] = b_str;
      else if (extsize & SIZE_LONG)     realip->qualifiers[0] = l_str;
      else if (extsize & SIZE_EXTENDED) realip->qualifiers[0] = x_str;
      ...
      ext = realip->qualifiers[0][0];
    }
```
   On 68000 the priority is **W > B > L**. So unsized `move` → `.w`, unsized `bra` (`SBW`) → `.w`, unsized `btst #n,Dn` (`L` only) → `.l`, unsized `lea` (`L`) → `.l`. `IFL_UNSIZED` is set permanently on the real ip and is what enables `opt_bra` on later passes.
   (The syntax module also has `set_default_qualifiers()` at `cpu.c:4914-4920` returning `"w"` for non-ColdFire, used for macro-argument defaults.)
4. Unsized-instruction check: an extension on a `SIZE_UNSIZED` mnemonic → `cpu_error(35)` (WARNING) and the extension is dropped (unless `-guess-ext`).
5. Stage-C size advance (§1.6).
6. `if (realip->ext.un.real.orig_ext < 0) realip->ext.un.real.orig_ext = (signed char)ext;` (`3745-3746`) — recorded once, for the `-showopt` messages.
7. `ipslot=0; ip = copy_instruction(realip); extflags = optimize_instruction(ip,sec,pc,0);` (`3777-3779`) — **optimisation runs on a shallow copy**, so the real operands are untouched during `resolve()`. Only `realip->code`, `realip->qualifiers[0]`, and `realip->ext.un.real.{flags,orig_ext,last_size}` are mutated by `instruction_size` itself.
8. `size = iplist_size(ip);` then
```c
  if (!(extflags & IFL_RETAINLASTSIZE))
    realip->ext.un.real.last_size = size;  /* remember size for next pass */
```

`instruction_ext` (`cpu.h:23-39`):
```c
#define HAVE_INSTRUCTION_EXTENSION 1
typedef struct {
  union {
    struct { unsigned char flags; signed char last_size; signed char orig_ext; char unused; } real;
    struct { struct instruction *next; } copy;
  } un;
} instruction_ext;
#define IFL_RETAINLASTSIZE    1   /* retain current last_size value */
#define IFL_UNSIZED           2   /* instruction had no size extension */
```
`init_instruction_ext` (`cpu.c:187-192`) sets `flags=0, last_size=-1, orig_ext=-1`. **The union is overloaded**: `optimize_instruction` writes `ip->ext.un.copy.next = NULL` at `cpu.c:2413`, destroying `flags`/`last_size`. That's why `eval_instruction` saves them at entry (`4249-4250`) and restores them at `eval_done` (`4531-4532`).

### 3.3 Size computation

```c
static size_t oper_size(instruction *ip,operand *op,struct optype *ot)   /* cpu.c:3537 */
{
  if (ot->flags & OTF_NOSIZE)                                      return 0;
  else if (mode==MODE_An16Disp ||
           (mode==MODE_Extended && (reg==REG_PC16Disp || reg==REG_AbsShort)))  return 2;
  else if (mode==MODE_Extended && reg==REG_AbsLong) {
    if (ot->flags & OTF_BRANCH) return branch_size(<ext>);         /* b/s=0, l=4, else 2 */
    else                        return 4;
  }
  else if (mode==MODE_Extended && reg==REG_Immediate) {
    switch (<ext>) { case 'b': case 'w': return 2;
                     case 'l': case 's': return 4;
                     case 'q': case 'd': return 8;
                     case 'x': case 'p': return 12; }
  }
  else if (mode==MODE_An8Format || (mode==MODE_Extended && reg==REG_PC8Format)) {
    /* 2, plus 2/4 for FW_BDSize, plus 2/4 for FW_IndSize */
  }
  return 0;
}

static size_t iplist_size(instruction *ip)                          /* cpu.c:3600 */
{
  do {
    if (ip->code >= 0) {
      size += S_OPCODE_SIZE(mnemonics[ip->code].ext.size) << 1;
      for (i=0; i<MAX_OPERANDS && ip->op[i]!=NULL; i++)
        size += oper_size(ip,ip->op[i],&optypes[mnemonics[ip->code].operand_type[i]]);
    }
  } while (ip = ip->ext.un.copy.next);
  return size;
}
```
Note `ip->code == -1` means "deleted instruction, 0 bytes", and the loop over operands stops at the first `NULL` — several optimisations exploit that by nulling `op[0]`.

### 3.4 Operand-level optimisations — `optimize_oper()` (`cpu.c:1836-2175`)

Preamble (`1847-1861`):
```c
  if (!(op->flags & FL_DoNotEval)) eval_oper(op,sec,pc,final);
  if ((op->flags & FL_NoOpt) == FL_NoOpt) return;
  bopt = !(op->flags & FL_NoOptBase);
  oopt = !(op->flags & FL_NoOptOuter);
  size16[0] = op->extval[0]>=-0x8000 && op->extval[0]<=0x7fff;
  size16[1] = op->extval[1]>=-0x8000 && op->extval[1]<=0x7fff;
  pcdisp = op->extval[0] - cpc;
  pcdisp16 = (op->base[0]==NULL) ? 0 : (pcdisp>=-0x8000 && pcdisp<=0x7fff);
  undef = (op->base[0]==NULL) ? 0 : EXTREF(op->base[0]);
```
`cpc` is passed by `optimize_instruction` as `pc + (S_OPCODE_SIZE(mnemo->ext.size) << 1)` (`cpu.c:2354`) — the address of the first extension word.

`eval_oper()` (`cpu.c:1731-1759`):
```c
  for (i=0; i<2; i++) {
    op->base[i] = NULL;
    if (type_of_expr(op->value[i]) == NUM) {
eval:
      if (!eval_expr(op->value[i],&op->extval[i],sec,pc)) {
        op->basetype[i] = find_base(op->value[i],&op->base[i],sec,pc);
        if (op->basetype[i] == BASE_ILLEGAL) {
          if (op->flags & FL_BaseReg) { if (fix_basereg(op,final)) goto eval; }
          if (final) general_error(38);  /* illegal relocation */
        }
      }
      op->flags |= FL_ExtVal0 << i;
    }
    else {
      op->extval[i] = 0x7fffffff;  /* dummy to prevent immediate opt. */
      op->flags &= ~(FL_ExtVal0 << i);
    }
  }
```
`type_of_expr(NULL)` returns 0 (`expr.c`), so a missing `value[i]` leaves `extval[i]` = 0x7fffffff and clears `FL_ExtVal*`. `find_base` (`expr.c:1379-1396`) returns `BASE_NONE`/`BASE_OK`/`BASE_PCREL`/`BASE_ILLEGAL`; `BASE_PCREL` is produced for `sym1 - sym2` where `sym2` is a local label in the current section (`expr.c:1361-1372`).

The 68000-relevant transformations, in the order they are tested:

**(a) `(0,An) → (An)`** — `cpu.c:1865-1876`:
```c
    if (op->mode==MODE_An16Disp) {
      if (opt_disp && !op->base[0] && op->extval[0]==0 &&
          (ot->modes & (1<<AM_AnIndir))) {
        op->mode = MODE_AnIndir;
        ...
        op->value[0] = NULL;
      }
```
Requires `bopt` (no explicit displacement size), `opt_disp`, an absolute zero, and that the optype allows mode 2. Frees the expression only when `final`.

**(b) `(d16,An) → (bd32,An,ZDn.w)`** (`1877-1886`), **`(d16,PC) → (bd32,PC,…)`** (`1889-1901`), **`(d8,An/PC,Rn) → (bd,…)`** (`1903-1931`) — all gated on `cpu_type & (m68020up|cpu32)`. **Never fire on 68000**; on 68000 the operand simply stays and `write_ea_ext` reports `cpu_error(29)` if the value doesn't fit. (Confirmed: `move.b 40000(a0),d0` → error 2030.)

**(c) `abs.w → abs.l`** — `cpu.c:1933-1949`:
```c
    else if (op->mode==MODE_Extended && op->reg==REG_AbsShort &&
             (ot->modes & (1<<AM_AbsLong))) {
      if (!op->base[0] && !size16[0]) {
        op->reg = REG_AbsLong;                       /* absval.w --> absval.l */
      }
      else if (op->base[0]) {
        if (typechk && LOCREF(op->base[0])) {
          op->reg = REG_AbsLong;                     /* label.w --> label.l */
          if (final) cpu_error(22);  /* need 32 bits to reference a program label */
        }
      }
    }
```
Gated on `bopt`, so an explicit `.w` (which sets `FL_NoOptBase`) blocks it — that is why `move.w $8000.w,d0` produces `cpu_error(32)` "absolute short address out of range" instead of being widened.

**(d) `abs.l → abs.w`, and `label.l → (d16,PC)`** — `cpu.c:1951-1967`:
```c
    else if (op->mode==MODE_Extended && op->reg==REG_AbsLong) {
      if (opt_abs && !op->base[0] && size16[0] &&
          (ot->modes & (1<<AM_AbsShort))) {
        op->reg = REG_AbsShort;                      /* absval.l --> absval.w */
      }
      else if (opt_pc && op->base[0] && (ot->modes & (1<<AM_PC16Disp))) {
        if (!undef && pcdisp16 && op->base[0]->sec==sec) {
          op->reg = REG_PC16Disp;                    /* label.l --> d16(PC) */
        }
      }
    }
```
Key consequences on 68000:
- `abs.l → abs.w` only for **constants** (`base[0]==NULL`), never for labels. Verified: `move.w $1000,d0` → `3038 1000`; `move.w label1,d0` → `303a ffc0` (PC-relative, not abs.w); `tst.w label1` → `4a79 00001000` (abs.l, because `AD` has no `AM_PC16Disp` bit).
- `label.l → (d16,PC)` only for local labels (`LOCREF`, not `EXTREF`) in the *same section*, within ±32K of `cpc`, and only when the optype includes bit 9. `AY/DA/DN/MA/MI/CT/MR/BS` do; `AL/AM/AD/AC/MS` do not.
- These two are an `if/else`, so a constant that fits 16 bits takes the `abs.w` branch and never reaches the PC branch (it couldn't anyway — the PC branch requires `base[0]`).

**(e)** Everything in `cpu.c:1970-2174` is gated on `FW_FullFormat`, which on 68000 is only set by `parse_operand`'s fallback path that also sets `FL_020up` — and `write_ea_ext` (`3944-3952`) errors on those. Effectively dead on 68000.

### 3.5 Instruction-level optimisations — `optimize_instruction()` (`cpu.c:2318-3534`)

Structure:
```
2335-2352  stage-D mnemonic generalisation
2354-2359  cpc = pc + opcode_words*2;  (phxass: pc = cpc)
2361-2365  JMP/JSR (label,PC) -> FL_NoOpt on operand 0
2367-2369  optimize_oper() on every operand              <-- runs even when no_opt!
2372-2411  MOVEM / FMOVEM register-list side detection & swap
2413       ip->ext.un.copy.next = NULL
2415-2416  if (no_opt) return ipflags;
2418-2610  STAGE 1: opt_mul only
2612-2618  re-read mnemo/oc/ext
2620-3508  STAGE 2: one big else-if chain, dispatched on ip->code / oc
3510-3520  opt_immaddr (separate `if`, not part of the chain)
3525-3531  optimize_oper() again over the whole ip chain
3533       return ipflags
```

Locals captured once at `2421-2424`:
```c
  oc = mnemo->ext.opcode[0];
  if (ip->op[0]) { val = ip->op[0]->extval[0]; abs = ip->op[0]->base[0]==NULL; }
```
`val`/`abs` are **not re-read** after stage 1 and are **uninitialised when `op[0]==NULL`** (declared `int abs,i;` at 2330). Every read is in practice guarded by an opcode or `op[0]!=NULL` test, so initialising both to 0 is behaviourally equivalent on 68000.

`ext` is re-read at `2616-2617` from `ip->qualifiers[0]`, then never updated again even when a branch changes the qualifier — so the `opt_immaddr` test at 3510 sees the *stage-2-entry* extension.

`ip->ext.un.copy.next` chains additional generated instructions; `MAX_IP_COPIES 4` (`cpu.c:147`), `ipslot` reset to 0 by the caller.

#### MOVEM register-list disambiguation (`cpu.c:2372-2380`)
```c
  if (!strcmp(mnemo->name,"movem") &&
      mnemo->ext.place[0]==D16 && mnemo->ext.place[1]==SEA &&
      ip->op[0]->base[0]!=NULL && ip->op[1]->base[0]==NULL) {
    /* destination operand seems to be the register list - swap them */
    ip->op[0]->reg = REG_AbsLong;
    ip->op[1]->reg = REG_RnList;
    ip->code += 2;  /* take matching mnemonic with swapped operands */
    mnemo = &mnemonics[ip->code];
  }
```

#### MOVEM → MOVE (`cpu.c:2715-2781`)
```c
  else if ((opt_gen || opt_movem) && !strcmp(mnemo->name,"movem")) {
    int o = (oc & 0x0400) ? 1 : 0;              /* which side holds the list */
    if (ip->op[o]->mode==MODE_Extended &&
        (ip->op[o]->reg==REG_RnList || ip->op[o]->reg==REG_Immediate) &&
        ip->op[o]->base[0]==NULL && !(ip->op[o]->flags & FL_NoOpt)) {
      taddr list = ip->op[o]->extval[0];
      int regs = cntones(list,16);
      if (regs == 0) { ip->code = -1; }        /* delete */
      else if (regs == 1) {
        if ((opt_movem || (!(list&0xff) && o==1)) && !aindir_in_list(ip->op[o^1],list)) {
          signed char r = bfffo(list,0,16);
          ip->code = OC_MOVE;
          ip->op[o]->mode = REGisAn(r) ? MODE_An : MODE_Dn;
          ip->op[o]->reg = REGget(r);
        }
      }
      else if (regs==2 && opt_speed && ...) { /* 68020+/68040 only */ }
    }
  }
```
With defaults (`opt_gen=1, opt_movem=0`) on 68000 this fires only for `movem <ea>,An` (`o==1`, list has no data register). Verified: `movem.l (sp)+,a0` → `205f` (`MOVEA.L (A7)+,A0`); `movem.l a0,-(sp)` stays `48e7 0080`. Empty lists are always deleted.

#### MOVE/MOVEA immediate (`cpu.c:2620-2703`)
```c
  if ((ip->code==OC_MOVE || oc==0x0040) &&
      ip->op[0]->mode==MODE_Extended && ip->op[0]->reg==REG_Immediate) {
    if (opt_moveq && abs && ext=='l' && val>=-0x80 && val<=0x7f && ip->op[1]->mode==MODE_Dn) {
      ip->code = OC_MOVEQ;  ip->qualifiers[0] = l_str;                   /* -> MOVEQ */
    }
    else if (opt_gen && abs && val==0 && !(oc&0x0040) &&
             ((cpu_type & (m68010up|mcf|cpu32)) || opt_clr)) {
      ip->code = OC_CLR;  ...  ip->op[0] = ip->op[1]; ip->op[1] = NULL;  /* -> CLR */
    }
    else if (opt_moveq && ... (cpu_type & (mcfb|mcfc))) { /* MOV3Q */ }
    else if (opt_st && abs && ext=='b' && !(oc&0x0040) && (val&0xff)==0xff) { /* -> ST */ }
    else if (opt_pea && ip->op[1]->mode==MODE_AnPreDec && ip->op[1]->reg==7 && ext=='l') {
      if (abs && val>=-0x8000 && val<=0x7fff) { ip->op[0]->reg = REG_AbsShort; ip->code = OC_PEA; }
      else if (cpu_type & (m68000|m68010|m68020|m68030|cpu32)) { ip->op[0]->reg = REG_AbsLong; ip->code = OC_PEA; }
      ip->op[1] = NULL;
    }
    else if (opt_gen && oc==0x0040) {
      if (abs && val==0) { ip->code = OC_SUBA; ip->qualifiers[0]=l_str;   /* MOVEA #0,An -> SUBA.L An,An */
                           ip->op[0]->mode = MODE_An; ip->op[0]->reg = ip->op[1]->reg; }
      else if (!abs && ext=='l') { ip->code = OC_LEA; ip->op[0]->reg = REG_AbsLong; }  /* -> LEA */
    }
  }
```
**On 68000 the `move #0,<ea> → clr` branch does not fire** (`opt_clr=0`, `cpu_type & m68010up == 0`). Verified: `move.b #0,d0` → `103c 00ff`… no: `103c 0000`, and `move.w #0,(a0)` → `30bc 0000`. Because this is an `if/else` chain and the CLR branch's *condition* is what fails, control falls to the later branches; none apply, so the MOVE is emitted unchanged.

`OC_MOVE` is the index of the `"move",{DA,AD}` row (`code_tab`, `cpu.c:129`), and `oc==0x0040` covers the MOVEA rows (`move {AY,A_}`, `movea {AY,A_}`).

#### CLR.L Dn → MOVEQ (`cpu.c:2798-2813`)
```c
  else if (opt_gen && oc==0x4200 && ip->op[0]->mode==MODE_Dn && ext=='l') {
    ip->code = OC_MOVEQ;  ip->qualifiers[0] = l_str;
    if (final) { ip->op[1] = ip->op[0]; ip->op[0] = new_operand(); ...#0... }
    else ip->op[0] = NULL;   /* size = 2 either way */
  }
```
Verified: `clr.l d1` → `7200`.

#### ANDI/AND (`2815-2854`), ORI/OR/EORI #0 (`2856-2868`), EORI #-1 (`2870-2882`)
```c
  else if (opt_gen && abs && (oc==0xc000 || oc==0x0200) && <op0 immediate>) {
    ... else if ((val==0xff && ext=='b') || (val==0xffff && ext=='w') ||
                 (val==0xffffffff && ext=='l')) { ip->code = OC_TST; ... }
    else if (val==0 && ((cpu_type & (m68010up|mcf|cpu32)) || opt_clr)) { ip->code = OC_CLR; ... }
  }
  else if (opt_gen && abs && val==0 && (oc==0x8000 || oc==0x0000 || oc==0x0a00) && <op0 immediate>) {
    ip->code = OC_TST; ...
  }
  else if (opt_gen && abs && oc==0x0a00) {
    if ((ext=='b' && (val&0xff)==0xff) || (ext=='w' && (val&0xffff)==0xffff) ||
        (ext=='l' && val==-1)) { ip->code = OC_NOT; ... }
  }
```
Note `val==0xffffffff` compared against a signed `taddr` is `val == -1` in practice; the `.l` case is written as an unsigned literal in the source but the compiler narrows it. Verified: `and.w #$ffff,d0` → `4a40`; `ori.w #0,d0` → `4a40`; `eori.b #-1,d0` → `4600`; `andi.l #0,d0` stays `0280 00000000` on 68000.

#### ADDQ/SUBQ and ADDA/SUBA → LEA (`cpu.c:2884-2921`)
```c
  else if ((oc==0x0600 || oc==0xd000 || oc==0xd0c0 ||
            oc==0x0400 || oc==0x9000 || oc==0x90c0)) {
    if (ip->op[0]->mode==MODE_Extended && ip->op[0]->reg==REG_Immediate && abs) {
      if (opt_quick && val>=1 && val<=8) {
        ip->code = (oc&0x4200) ? OC_ADDQ : OC_SUBQ;
      }
      else if ((oc&0x90c0) == 0x90c0) {  /* ADDA/SUBA */
        if (!(oc & 0x4000)) val = -val;
        if (opt_gen && val == 0) { ip->code = -1; }
        else if (opt_lea && val>=-0x8000 && val<=0x7fff) {
          ip->qualifiers[0] = l_str;
          ip->code = OC_LEA;
          ip->op[0]->mode = MODE_An16Disp;
          ip->op[0]->reg = ip->op[1]->reg;
          if (!(oc&0x4000) && final) { free_expr(...); ip->op[0]->value[0] = number_expr(val); }
          else { ip->op[0]->flags |= FL_DoNotEval; ip->op[0]->extval[0] = val; }
        }
      }
    }
  }
```
`FL_DoNotEval` is essential: it stops `optimize_oper` from re-evaluating `value[0]` and overwriting the synthesised `extval[0]`. Verified: `adda.l #$1234,a0` → `41e8 1234`; `suba.l #100,a0` → `41e8 ff9c`; `adda.l #$70000,a0` stays `d1fc 00070000`.

#### LEA (`cpu.c:2923-2994`)
```c
  else if (oc==0x41c0) {
    if (ip->op[0]->mode==MODE_An16Disp && abs) {
      if (opt_gen && ip->op[0]->reg==ip->op[1]->reg && val==0)   ip->code = -1;   /* lea (0,An),An */
      else if (opt_lquick && ip->op[0]->reg==ip->op[1]->reg && val!=0 && val>=-8 && val<=8) {
        if (val < 0) { ip->code = OC_SUBQ; val = -val;
                       if (final) { free_op_exp(ip->op[0]); ip->op[0]->value[0]=number_expr(val); } }
        else ip->code = OC_ADDQ;
        ip->qualifiers[0] = l_str;
        ip->op[0]->mode = MODE_Extended;  ip->op[0]->reg = REG_Immediate;
      }
      else if (opt_gen && (val<-0x8000 || val>0x7fff) && !(cpu_type & (m68020up|cpu32))) {
        if (ip->op[0]->reg == ip->op[1]->reg) {   /* LEA (d32,An),An -> ADDA.L #d32,An */
          ip->code = OC_ADDA; ip->op[0]->mode = MODE_Extended;
          ip->op[0]->reg = REG_Immediate; ip->op[0]->flags |= FL_NoOpt;
        }
        else {                                    /* -> MOVEA.L Am,An ; ADDA.L #d32,An */
          ip2 = ip_doubleop(OC_ADDA,l_str, MODE_Extended,REG_Immediate,FL_NoOpt,0,ip->op[0]->value[0],
                                           MODE_An,ip->op[1]->reg,FL_NoOpt,0,NULL);
          ip->code = OC_MOVEA; ip->op[0]->mode = MODE_An;
          ip->ext.un.copy.next = ip2;
        }
        ip->qualifiers[0] = l_str;
        if (final) cpu_error(47); /* lea-displacement out of range, changed */
      }
    }
    else if (opt_gen && ip->op[0]->mode==MODE_AnIndir && ip->op[0]->reg==ip->op[1]->reg)
      ip->code = -1;                                /* lea (An),An */
    else if (opt_gen && abs && val==0 && ip->op[0]->mode==MODE_Extended &&
             (ip->op[0]->reg==REG_AbsShort || ip->op[0]->reg==REG_AbsLong)) {
      ip->code = OC_SUBA; ip->qualifiers[0] = l_str;   /* LEA 0,An -> SUBA.L An,An */
      ip->op[0]->mode = MODE_An; ip->op[0]->reg = ip->op[1]->reg;
    }
  }
```
Note `val>=-8 && val<=8` includes 0, but `val!=0` excludes it (handled by the delete case above). Verified: `lea 8(a0),a0` → `5088`; `lea 0(a0),a0` and `lea (a0),a0` → deleted; `lea 0,a0` → `91c8`; `lea $70000(a0),a0` → `d1fc 00070000` with warning 2048; `lea $70000(a1),a0` → `2049 d1fc 00070000` with warning 2048.

#### CMP/CMPI/CMPA #0 → TST (`cpu.c:3021-3041`)
```c
  else if (oc==0x0c00 || oc==0xb000 || oc==0xb0c0) {
    if (opt_gen && abs && val==0 && <op0 immediate>) {
      if (oc!=0xb0c0 || (cpu_type & (m68020up|cpu32|mcf))) {
        if (oc == 0xb0c0) { ip->code = OC_TST + 1; ip->qualifiers[0] = l_str; }
        else              ip->code = OC_TST;
        ...
      }
    }
  }
```
`OC_TST+1` is the `tst {DA}` 020+ row — never used on 68000; `cmpa #0,An` is not optimised on 68000.

#### ASL/LSL #1 → ADD (`cpu.c:3043-3066`)
```c
  else if ((((oc&0xf1ff)==0xe100 && opt_gen) ||
            ((oc&0xf1ff)==0xe108 && opt_lsl)) && !(cpu_type&(m68060|mcf))) {
    if ((oc&0x0e00) == 0x0200) val = 1;  /* ASL/LSL Dn (missing immediate operand assumed as 1) */
    if (val == 1) {
      ip->code = OC_ADD;
      ip->op[0]->mode = MODE_Dn;
      if (!(oc&0x0e00)) ip->op[0]->reg = ip->op[1]->reg;
    }
    else if (opt_speed && opt_lsl && val==2 && (ext=='b' || ext=='w')) { /* two ADDs */ }
  }
```
**This contains a real bug that you must replicate byte-for-byte.** For the register-only form (`"asl",{D_}`, opcode `0xe300`) the mask `oc&0x0e00 == 0x0200` is non-zero, so `op[0]->reg` is not fixed up and `op[1]` is `NULL`; the resulting `ADD` gets destination register 0. Empirically verified with the reference binary:

```
    asl.l d3      ->  d083   ; ADD.L D3,D0   (!!)
    asl.l #1,d3   ->  d683   ; ADD.L D3,D3   (correct)
    asl.w d5      ->  d045   ; ADD.W D5,D0   (!!)
    lsl.l d3      ->  e38b   ; unchanged (opt_lsl=0)
    asr.l d3      ->  e283   ; unchanged
```

#### JMP/JSR → BRA/BSR (`cpu.c:3294-3334`)
```c
  else if ((oc==0x4ec0 || oc==0x4e80) && !abs) {
    if (opt_pc && !(ip->op[0]->flags & FL_NoOpt) &&
        ip->op[0]->mode==MODE_Extended &&
        (ip->op[0]->reg==REG_AbsLong || ip->op[0]->reg==REG_PC16Disp) &&
        LOCREF(ip->op[0]->base[0]) && ip->op[0]->base[0]->sec==sec) {
      taddr diff = val - cpc;
      if (lastsize==0 || (diff==0 && (oc & 0x40))) {
        ip->code = -1;  /* delete a JMP to following location */
      }
      else if (diff>=-0x8000 && diff<=0x7fff) {
        if (diff>=-0x80 && diff<=0x7f) {
          if ((lastsize==2 && diff==0) || (lastsize==4 && diff==2))
            ip->qualifiers[0] = w_str;
          else
            ip->qualifiers[0] = b_str;
          ip->code = (oc & 0x40) ? OC_BRA : OC_BSR;
          ip->op[0]->reg = REG_AbsLong;
        }
        else {
          ip->qualifiers[0] = w_str;
          ip->code = (oc & 0x40) ? OC_BRA : OC_BSR;
          ip->op[0]->reg = REG_AbsLong;
        }
      }
    }
    else if (opt_sc && ... EXTREF(base)) { ip->op[0]->reg = REG_PC16Disp; }
  }
```
Note this is gated on `opt_pc`, not `opt_bra`. `0x4ec0 & 0x40 == 0x40` → BRA; `0x4e80 & 0x40 == 0` → BSR. Also note `cpu.c:2361-2365` unconditionally sets `FL_NoOpt` on `JMP/JSR (label,PC)` so those are never converted. Verified: `jmp label1` → `60b2`, `jsr label1` → `61b4`.

#### Branch sizing (`cpu.c:3336-3479`) — the core of pass-to-pass convergence
```c
  else if ((oc & 0xf000)==0x6000 && !abs) {
    if (opt_bra && ((ipflags&IFL_UNSIZED) || opt_allbra) &&
        LOCREF(ip->op[0]->base[0]) && ip->op[0]->base[0]->sec==sec) {
      taddr diff = val - cpc;
      int resolvewarn = (sec->flags&RESOLVE_WARN)!=0;

      switch (lastsize) {
        case 0:
          if (diff != -2) ip->qualifiers[0] = b_str;
          else            ip->code = -1;
          break;
        case 2:
          if (diff==0 && oc!=0x6100 && !resolvewarn)      ip->code = -1;
          else if (diff<-0x80 || diff>0x7f || diff==0)    ip->qualifiers[0] = w_str;
          else                                            ip->qualifiers[0] = b_str;
          break;
        case 4:
          if (diff==2) {
            if (oc!=0x6100 && !resolvewarn) ip->code = -1;
            else                            ip->qualifiers[0] = w_str;
          }
          else if (diff>=-0x80 && diff<=0x80 && !resolvewarn) {
            ip->qualifiers[0] = b_str;
          }
          else if (diff<-0x8000 || diff>0x7fff) {
            if (cpu_type & (m68020up|cpu32|mcfb|mcfc)) ip->qualifiers[0] = l_str;
            else {
              ip->qualifiers[0] = emptystr;
              ipflags |= IFL_RETAINLASTSIZE;
              if (oc < 0x6200) {           /* BRA/BSR -> JMP/JSR */
                ip->code = (oc==0x6000) ? OC_JMP : OC_JSR;
                if (final) cpu_error(46);
              }
              else {                        /* Bcc -> B!cc *+8 ; JMP */
                ip2 = ip_singleop(OC_JMP,emptystr,MODE_Extended,REG_AbsLong,
                                  FL_NoOpt,0,ip->op[0]->value[0]);
                ip->code += (oc&0x0100) ? -2 : 2;   /* negate branch condition */
                ip->qualifiers[0] = b_str;
                ip->op[0]->flags |= FL_NoOpt;
                ip->ext.un.copy.next = ip2;
                if (final) {
                  ip->op[0]->value[0] = make_expr(ADD,curpc_expr(),
                          number_expr(phxass_compat ? 6 : 8));
                  cpu_error(46);
                }
              }
            }
          }
          else ip->qualifiers[0] = w_str;
          break;
        case 6:  /* only reachable on 020+ */ ... break;
        default: if (ext == '\0') ip->qualifiers[0] = w_str; break;
      }
      ...
    }
    else if (opt_branop && oc!=0x6100 && val-cpc==0 &&
             (ext=='b' || ext=='s') && LOCREF(...) && ...sec==sec) {
      ip->qualifiers[0] = emptystr;
      ip->code = OC_NOOP;         /* opcode 0x4dd6 == LEA (A6),A6 */
      ip->op[0] = NULL;
      if (final) cpu_error(57);
    }
    else if (opt_brajmp && ip->op[0]->base[0]->sec!=sec && LOCREF(...)) { ... }
  }
```

**Answers to your hysteresis question.** `lastsize` is the *previous pass's total instruction size* (2 for `.b`, 4 for `.w`, 0 for deleted, −1 on the first pass). There is **no monotonic latch** — a branch can shrink and grow freely between passes. The convergence machinery is:

1. **Pass 1**: `lastsize == -1` → `default:` case. But `instruction_size` has already assigned `.w` (because `SBW` prefers `SIZE_WORD`), so `ext != '\0'` and nothing changes → size 4.
2. **Asymmetric thresholds prevent 2-cycle oscillation.** Let `N` be the number of bytes between the branch and its target. When currently `.w`, `diff = N+2`; the `.b` test is `diff <= 0x80`, i.e. `N <= 126`. When currently `.b`, `diff = N`; staying `.b` requires `N <= 0x7f`, i.e. `N <= 127`. The `+2` slack in the `.w→.b` test exactly accounts for the shrink, so no straddling value flips forever. Verified: `N=126` → `607e` (.b), `N=127` → `6000 0081` (.w).
3. **Zero-distance deletion**: a `bra` to the immediately following instruction is *deleted entirely*. Pass 2 (`lastsize==4, diff==2`) → `code = -1`, size 0; pass 3 (`lastsize==0, diff==-2`) → stays deleted. `bsr` (`oc==0x6100`) is never deleted; it becomes `.w` with displacement 2. Verified: `bra n1 / n1:` emits nothing, `bsr n2 / n2:` emits `6100 0002`.
4. **`IFL_RETAINLASTSIZE`** is the guard for the "out of 16-bit range on 68000" translation: `instruction_size` skips the `last_size` update (`cpu.c:3783-3784`), so the next pass re-enters `case 4` and performs the same translation deterministically instead of seeing the new (6- or 8-byte) size and flip-flopping. It is returned as a local flag only — it is *not* stored back into `realip->ext.un.real.flags`.
5. **`resolvewarn`** (`sec->flags & RESOLVE_WARN`) is the escape hatch for genuinely oscillating cases (mutually dependent branches). It becomes true for an atom whose size has changed more than `MAXSIZECHANGES` (5) times during safe-mode passes (`vasm.c:235-243`), and it forces the branch to the *larger* form and forbids deletion. It is set per-atom, only around that atom's `atom_size()` call, and again in `assemble()` before `eval_instruction()` (`vasm.c:305-306`, cleared at `vasm.c:413`).
6. `opt_branop`'s "NOP" is **not `0x4e71`** — `OC_NOOP` is the `" no-op"` pseudo-mnemonic at `opcodes.h:1244` with opcode `0x4dd6` = `LEA (A6),A6` (a flag-preserving no-op). Verified: `bra.b *+2` → `4dd6` with warning 2058.

#### `opt_immaddr` (`cpu.c:3510-3520`)
```c
  if (opt_immaddr && abs && ext=='l' && ip->op[0]!=NULL &&
      ip->op[0]->mode==MODE_Extended && ip->op[0]->reg==REG_Immediate &&
      ip->op[1]!=NULL && ip->op[1]->mode==MODE_An &&
      val>=-0x8000 && val<=0x7fff &&
      !(cpu_type & mcf) && (mnemonics[ip->code].ext.size & SIZE_WORD) &&
      (mnemonics[ip->code].ext.opcode[0] & 0xfeff) != 0x5000) {
    ip->qualifiers[0] = w_str;
  }
```
Verified: `move.l #$1234,a0` → `307c 1234` (MOVEA.W), `cmpa.l #$1234,a0` → `b0fc 1234`.

#### Second operand pass (`cpu.c:3525-3531`)
```c
  for (ip=iplist; ip; ip=ip->ext.un.copy.next) {
    if (ip->code >= 0)
      for (i=0; i<MAX_OPERANDS && ip->op[i]!=NULL; i++)
        optimize_oper(ip->op[i],&optypes[mnemonics[ip->code].operand_type[i]],sec,pc,cpc,final);
  }
```
This re-runs `optimize_oper` with the *new* mnemonic's optypes — this is how `movea.l #label,An → lea label,An → lea (d16,PC),An` completes in a single call (verified: `41fa 0004`). It also re-runs `eval_oper` unless `FL_DoNotEval` is set.

---

## 4. Encoding — `eval_instruction()` (`cpu.c:4244-4535`)

```c
  ipslot = 0;
  optimize_instruction(ip,sec,pc,1);           /* mutates the REAL ip */
  if (db->size = iplist_size(ip)) d = db->data = mymalloc(db->size);
  else { db->data = NULL; goto eval_done; }
  do {
    if (ip->code >= 0) {
      mnemonic *mnemo = &mnemonics[ip->code];
      char ext = ...;
      uint16_t sz = ((mnemo->ext.size & SIZE_MASK) == SIZE_UNSIZED) ? SIZE_UNSIZED : lc_ext_to_size(ext);
      unsigned char *dbstart = d;
      if (mnemo->ext.available & malias) cpu_error(33);
      /* copy opcode */
      *d++ = mnemo->ext.opcode[0] >> 8;         /* (fpu variants OR in the CP-ID) */
      *d++ = mnemo->ext.opcode[0] & 0xff;
      pc += 2;
      if (S_OPCODE_SIZE(mnemo->ext.size) > 1) { *d++ = opcode[1]>>8; *d++ = opcode[1]&0xff; pc += 2;
        if (S_OPCODE_SIZE(...) > 2) { *d++ = 0; *d++ = 0; pc += 2; } }
```
**`pc` tracks the current write position**: it is advanced by 2 per opcode word and by `newd - d` after every operand (`cpu.c:4522-4523`). This is what makes PC-relative displacements come out right (the 68000 base is the address of the extension word).

### 4.1 Size-field insertion (`cpu.c:4306-4383`)
```c
      switch (S_SIZEMODE(mnemo->ext.size)) {
        case S_NONE:  break;
        case S_STD:  W: *(dbstart+1)|=0x40;  L: *(dbstart+1)|=0x80;                     break;
        case S_STD1: B: *(dbstart+1)|=0x40;  W: |=0x80;  L: |=0xc0;                     break;
        case S_HI:   W: *dbstart|=0x02;      L: *dbstart|=0x04;                          break;
        case S_CAS:  B: *dbstart|=0x02;      W: |=0x04;  L: |=0x06;                      break;
        case S_MOVE: B: *dbstart|=0x10;      W: |=0x30;  L: |=0x20;                      break;
        case S_WL8:  L: *dbstart|=1;                                                     break;
        case S_LW7:  W: *(dbstart+1)|=0x80;                                              break;
        case S_WL6:  L: *(dbstart+1)|=0x40;                                              break;
        case S_MAC:  L: *(dbstart+2)|=8;                                                 break;
        case S_TRAP: W: *(dbstart+1)|=0x02;  L: |=0x03;                                  break;
        case S_EXT:  W: *(dbstart+3)|=0x40;  L: |=0x80;                                  break;
        case S_FP:   S:0x04 X:0x08 P:0x0c W:0x10 D:0x14 B:0x18  into *(dbstart+2);        break;
      }
```

### 4.2 Operand insertion (`cpu.c:4413-4524`)
```c
        switch (oii->mode) {
          case M_bfea: ... /* fall through */
          case M_ea:
            if (oii->flags & IIF_MASK)  *(dbstart+3) |= op->bf_width ? (1<<oii->pos) : 0;
            if (oii->flags & IIF_NOMODE) *(dbstart+1) |= REGget(op->reg);
            else                         *(dbstart+1) |= ((op->mode & 7) << 3) | REGget(op->reg);
            /* fall through */
          case M_noea:
            newd = write_ea_ext(db,d,op,ext,sec,pc);
            /* fall through */
          case M_nop: break;

          case M_high_ea:
            *(dbstart)   |= (REGget(op->reg) << 1) | ((op->mode & 4) >> 2);
            *(dbstart+1) |= (op->mode & 3) << 6;
            newd = write_ea_ext(db,d,op,ext,sec,pc);
            break;

          case M_branch:
            newd = write_branch(db,d,op,ext,sec,pc,(oii->flags&IIF_BCC)?1:0);
            break;

          case M_val0:
            if (op->base[0] == NULL) {
              taddr v = op->extval[0];
              if (oii->flags & IIF_MASK) { if (v == 0) v = 1 << oii->size; else if (v == (1<<oii->size)) v = 0; }
              else if (oii->flags & IIF_3Q) { if (v==0) v=-1; else if (v==-1) v=0; }
              if (oii->flags & IIF_REVERSE) v = reverse(v,oii->size);
              write_val(dbstart,oii->pos,oii->size,v,(oii->flags&IIF_SIGNED)!=0);
            }
            else cpu_error(24);  /* absolute value expected */
            break;

          case M_reg:
            if (op->mode<MODE_Extended || op->mode==MODE_FPn) {
              taddr r = (taddr)op->reg;
              if (oii->size>3 && op->mode==MODE_An) r += 8;
              write_val(dbstart,oii->pos,oii->size,r,0);
            } else ierror(0);
            break;
        }
        pc += newd - d;
        d = newd;
```
`M_high_ea` splits the 6-bit EA across the MOVE opcode's bits 11-6: `reg` into bits 11-9 (`<<1` on `dbstart` = bits 12-9 minus the low bit) and `mode` bits 2-0 into bits 8-6.

`write_val()` (`cpu.c:3789-3827`):
```c
  if (typechk) {
    if (sign) {
      if ((val > (1L << (size-1)) - 1) || (val < -(1L << (size-1)))) {
        if (val > 0 && val < (1L << size))
          cpu_error(27,val,-(1L<<(size-1)),(1L<<(size-1))-1,val-(1L<<size));  /* WARNING 2028 */
        else
          cpu_error(25,val,-(1L<<(size-1)),(1L<<(size-1))-1);                 /* ERROR 2026 */
      }
    } else {
      if ((utaddr)val > (1L << size) - 1)
        cpu_error(25,val,0,(1L<<size)-1);                                     /* ERROR 2026 */
    }
  }
  d += pos>>3;  pos &= 7;
  while (size > 0) {
    int shift = 8-pos-size;
    unsigned char v;
    if (shift > 0) v = (val << shift) & 0xff;
    else if (shift < 0) v = (val >> -shift) & 0xff;
    else v = val & 0xff;
    *d++ |= v;
    size -= 8-pos;  pos = 0;
  }
```
It **ORs** into the buffer (never clears), and it truncates silently after reporting. Verified: `moveq #200,d0` → warning 2028 ("using signed operand as unsigned: 200 (valid: -128..127), -56 to fix") and the byte 0xc8 is still written; `addq.l #9,d0` → error 2026 "operand value out of range: 9 (valid: 0..7)" (the range shown is post-`IIF_MASK`); `trap #16` → error 2026 "(valid: 0..15)".

`reverse()` (`cpu.c:489-501`) reverses the low `size` bits — used only by `D2R` (movem predecrement mask) and `E8R` (fmovem). Verified: `movem.l d0-d3/a0-a2,-(sp)` → `48e7 f0e0` (mask `0x070f` reversed), `movem.l (sp)+,d0-d3/a0-a2` → `4cdf 070f`. Note `movem #mask,-(An)` uses `D16` (place at `opcodes.h:877`), i.e. **not** reversed — the user's literal mask goes straight in.

### 4.3 Branch displacements — `write_branch()` (`cpu.c:3830-3916`)
```c
  if (!bcc && ext!='w' && ext!='l') ierror(0);
  if (op->base[0]) {
    if (is_pc_reloc(op->base[0],sec)) {   /* external, or label in another section */
      taddr addend = op->extval[0];
      switch (ext) {
        case 'b': case 's': addend--;  *(d-1) = addend & 0xff;  size=8; offset=1; break;
        case 'l': if (cpu_type & (m68020up|cpu32|mcfb|mcfc|m68881|m68882|m68851)) {
                    if (bcc) *(d-1) = 0xff;  offset = d-db->data;  d = setval(1,d,4,addend); size=32; }
                  else cpu_error(0);
                  break;
        case 'w': if (bcc) *(d-1) = 0;  offset = d-db->data;  d = setval(1,d,2,addend); size=16; break;
        default: cpu_error(34);
      }
      add_extnreloc(&db->relocs,op->base[0],addend,REL_PC,0,size,offset);
    }
    else {                                 /* same section: resolve now */
      taddr diff = op->extval[0] - pc;
      switch (ext) {
        case 'b': case 's':
          if (diff>=-0x80 && diff<=0x7f && diff!=0) *(d-1) = diff & 0xff;
          else cpu_error(28);              /* branch destination out of range */
          break;
        case 'l': ... (68020+ only, else cpu_error(0)) ...
        case 'w':
          if (diff>=-0x8000 && diff<=0x7fff) { if (bcc) *(d-1) = 0;  d = setval(1,d,2,diff); }
          else cpu_error(28);
          break;
      }
    }
  }
  else cpu_error(26);   /* label in operand required */
```
Two things to note: (a) `pc` here is the address *after* the opcode word, i.e. the correct 68000 branch base; (b) a **constant** branch destination (no base symbol) is an error 2027 — branches always require a label.

### 4.4 EA extension words — `write_ea_ext()` (`cpu.c:3929-4241`)

Guard for 020+ modes (`3944-3952`):
```c
    if (op->flags & FL_020up) {
      if (!(cpu_type & (m68020up|cpu32))) cpu_error(0);  /* instruction not supported */
      else if (op->flags & FL_noCPU32) { if (cpu_type & cpu32) cpu_error(0); }
    }
```

**`MODE_An16Disp`** (`3954-3975`):
```c
      if (op->base[0]) {
        rsize = 16;
        if ((EXTREF(op->base[0]) && op->reg!=sdreg) || op->basetype[0]==BASE_PCREL) rtype = REL_ABS;
        else if (op->basetype[0]==BASE_OK) rtype = REL_SD;
        else general_error(38);
      }
      if (rtype == REL_SD) { if (typechk && (op->extval[0]<0 || op->extval[0]>0xffff)) cpu_error(29); }
      else                 { if (typechk && (op->extval[0]<-0x8000 || op->extval[0]>0x7fff)) cpu_error(29); }
      d = write_extval(0,2,db,d,op,rtype);
```
Note the small-data (`REL_SD`) case checks an **unsigned** 0..0xffff range. For `-Fbin` this path never survives (relocations mean unresolved symbols, which `output_bin` rejects), but the range check still runs.

**`MODE_An8Format` brief** (`4025-4039`):
```c
        if (typechk && (op->extval[0]<-0x80 || op->extval[0]>0x7f)) cpu_error(29);
        if (op->base[0]) {
          rsize = 8;
          if (EXTREF(op->base[0]) || (LOCREF(op->base[0]) && op->basetype[0]==BASE_PCREL)) rtype = REL_ABS;
          else cpu_error(30);   /* absolute displacement expected */
        }
        *d++ = (op->format>>8) & 0xff;
        *d++ = op->extval[0] & 0xff;
```

**`REG_PC16Disp`** (`4044-4059`) — this is the direct-PC-displacement logic:
```c
      if (op->reg == REG_PC16Disp) {
        taddr disp = op->extval[0];
        if (op->base[0]) {
          if (is_pc_reloc(op->base[0],sec)) { rtype = REL_PC; rsize = 16; }
          else disp = op->extval[0] - pc;
        }
        if (typechk && (disp<-0x8000 || disp>0x7fff)) cpu_error(29);
        d = setval(1,d,2,disp);
      }
```

**`REG_PC8Format` brief** (`4119-4136`):
```c
          if (op->base[0]) {
            if (is_pc_reloc(op->base[0],sec)) { rtype=REL_PC; rsize=8; roffs++; op->extval[0]+=1; disp+=1; }
            else disp = op->extval[0] - pc;
          }
          if (typechk && (disp<-0x80 || disp>0x7f)) cpu_error(29);
          *d++ = (op->format>>8) & 0xff;
          *d++ = disp & 0xff;
```

**`REG_AbsShort`** (`4139-4149`): `if (typechk && (op->extval[0]<-0x8000 || op->extval[0]>0x7fff)) cpu_error(32);` then 2 bytes.
**`REG_AbsLong`** (`4151-4158`): no range check at all, 4 bytes.

**`REG_Immediate`** (`4160-4224`):
```c
          case 'b':
            if (op->flags & FL_ExtVal0) {
              roffs++;  rsize = 8;  *d++ = 0;  d = write_extval(0,1,db,d,op,rtype);
              if (typechk && (op->extval[0]<-0x80 || op->extval[0]>0xff)) cpu_error(36);
            } else cpu_error(37);
            break;
          case 'w':
            if (op->flags & FL_ExtVal0) {
              rsize = 16;  d = write_extval(0,2,db,d,op,rtype);
              if (typechk && (op->extval[0]<-0x8000 || op->extval[0]>0xffff)) cpu_error(36);
            } else cpu_error(37);
            break;
          case 'l':
            if (op->flags & FL_ExtVal0) { rsize = 32; d = write_extval(0,4,db,d,op,rtype); }
            else if (type_of_expr(op->value[0]) == FLT) { copy_float_exp(d,op,EXT_SINGLE); d += 4; }
            else cpu_error(37);
            break;
          /* 's','d','x','p' -> copy_float_exp, 4/8/12 bytes */
```
Byte immediates emit a leading zero byte then the value (word-aligned). The range test is **asymmetric** (`-0x80 .. 0xff`), accepting both signed and unsigned forms. `.l` has **no range check** (taddr is already 32-bit; over-wide literals are caught earlier by the expression evaluator as general warning 22 "target data type overflow").

`write_extval()` (`cpu.c:3919-3926`):
```c
  if (rtype==REL_ABS && op->basetype[num]==BASE_PCREL)
    op->extval[num] += d - db->data;  /* fix addend for label differences */
  return setval(1,d,size,op->extval[num]);
```

### 4.5 Answers to your specific encoding questions

**Which conditions produce error 2030 ("displacement out of range", `cpu_error(29)`, index 29, severity ERROR)?**
Error numbering: `FIRST_CPU_ERROR 2001` (`error.h:13`), `error()` at `error.c:71` indexes `cpu_err_out[n]` and prints `n + 2001`. Index 29 is `cpu_errors.h` line 31 (`"displacement out of range",ERROR`) — entry 27 spans two source lines, which is why the line/index offsets differ by 2 from there on. Confirmed by the docs: `-rangewarnings` demotes "2026, 2030, 2033 and 2037" and the code is `modify_cpu_err(WARNING,25,29,32,36,0)` (`cpu.c:4838`).

All eight sites, every one gated on `typechk` (default 1, cleared by `OPT T-`):

| site | condition |
|---|---|
| `cpu.c:3968` | `MODE_An16Disp`, small-data reloc: `extval[0] < 0 \|\| extval[0] > 0xffff` |
| `cpu.c:3972` | `MODE_An16Disp`, normal: `extval[0] < -0x8000 \|\| extval[0] > 0x7fff` |
| `cpu.c:3986` | `(bd16,An,Rn)` base disp out of signed 16 |
| `cpu.c:4006` | `([…],od16)` outer disp out of signed 16 |
| `cpu.c:4028` | `(d8,An,Rn)`: `extval[0] < -0x80 \|\| extval[0] > 0x7f` |
| `cpu.c:4057` | `(d16,PC)`: `disp < -0x8000 \|\| disp > 0x7fff` |
| `cpu.c:4082` | `(bd16,PC,Rn)` |
| `cpu.c:4100` | `([…PC…],od16)` |
| `cpu.c:4133` | `(d8,PC,Rn)`: `disp < -0x80 \|\| disp > 0x7f` |

On 68000 only rows 1, 2, 5, 6 and 9 are reachable. Verified: `move.b 40000(a0),d0` and `move.b ±200(a0,d1.w),d0` all give error 2030; with `-no-opt`, `lea $70000(a0),a0` gives error 2030 instead of being translated.

**A PC-relative operand with an ABSOLUTE (constant) displacement — `(label2-label1)-2(pc,d1.w)`:**

vasm 1.7h has **no `-nodpc` option and no direct-PC-displacement toggle** — I grepped the whole 1.7h tree; the concept does not exist here. What exists is the *implicit* rule embedded in `write_ea_ext`:

> If `op->base[0] == NULL` (the displacement expression collapsed to a pure constant), the constant is written **literally** into the displacement field, with **no `- pc` adjustment**. If `op->base[0] != NULL` and the symbol is in the same section (`!is_pc_reloc`), the displacement is computed as `extval[0] - pc`, where `pc` is the address of the extension/format word being written.

Parsing of `(label2-label1)-2(pc,d1.w)` works because of the "expression was only the first term" rewind at `cpu.c:1337-1341`: the leading `(` is first taken as an addressing-mode paren, the inner `label2-label1` is parsed, the closing `)` is seen, and `p` is reset to `start_term` so the *whole* `(label2-label1)-2` is re-parsed as one expression via `base_disp_and_ext`. Then `(pc,d1.w)` is parsed as the base+index, `disp_size` defaults to `EXT_BYTE` (index present), and the operand becomes `MODE_Extended/REG_PC8Format`, brief format.

Because both labels are local and resolvable, `eval_expr` succeeds in `eval_oper`, so `base[0]` stays `NULL` and `extval[0]` is the constant. `optimize_oper`'s PC8Format widening (`cpu.c:1917-1931`) is gated on `cpu_type & (m68020up|cpu32)` so nothing changes on 68000. The encoder writes the constant directly. Empirically:

```
    org $1000
label1: ...
label2: move.b (label2-label1)-2(pc,d1.w),d0   ->  103b 1058
        move.w (label2-label1)(pc),d0          ->  303a 005a
        move.w label1(pc),d0                   ->  303a ff9c
        lea    130(pc),a0                       ->  41fa 0082
```
`label2-label1 = 0x5a`, so `0x5a-2 = 0x58` appears verbatim as the d8. `(label2-label1)(pc)` writes `0x005a` verbatim. Only the symbolic form `label1(pc)` gets the `- pc` treatment (`0x1000 - 0x1064 = 0xff9c`). The out-of-range check applies to whichever value ends up in the field, producing error 2030.

---

## 5. Data directives, `m68k_data_operand`, alignment

**Which module does what.** The mot syntax module owns the directives; the cpu module owns the operand classification and the byte emission.

`syntax/mot/syntax.c:1465-1469` maps `dc` → `handle_d16`, `dc.b/w/l/q` → `handle_d8/d16/d32/d64`; `dcb`/`ds`/`blk`/`spc` go to `handle_block`/`handle_space` → `do_space` (`syntax.c:287-294`). `handle_data` (`syntax.c:587-621`):
```c
    if (OPSZ_BITS(size)==8 && (*s=='\"' || *s=='\'')) {
      if (db = parse_string(&opstart,*s,8)) { add_atom(0,new_data_atom(db,1)); s = opstart; }
    }
    if (!db) {
      op = new_operand();
      s = skip_operand(s);
      if (parse_operand(opstart,s-opstart,op,DATA_OPERAND(size))) {
        a = new_datadef_atom(OPSZ_BITS(size),op);
        if (!align_data) a->align = 1;
        add_atom(0,a);
      }
      else syntax_error(8);
    }
```

**There is no `cpu_data_size` and no `cpu_align` in 1.7h.** The cpu module contributes:
- `DATA_OPERAND(n)` → `m68k_data_operand(n)` (`cpu.h:89-90`, `cpu.c:227-241`):
```c
int m68k_data_operand(int bits)
{
  switch (bits) {
    case 8: return OP_D8;
    case 16: return OP_D16;
    case 32: return OP_D32;
    case 64: return OP_D64;
    case OPSZ_FLOAT|32: return OP_F32;
    case OPSZ_FLOAT|64: return OP_F64;
    case OPSZ_FLOAT|96: return OP_F96;
  }
  cpu_error(38,OPSZ_BITS(bits));  /* data obj. with n bits size are not supp. */
  return 0;
}
```
- `INST_ALIGN 2` and `DATA_ALIGN(n) ((n<=8)?1:2)` (`cpu.h:83,86`).
- `eval_data()` (`cpu.c:4538-4640`) — the emitter.

**Alignment behaviour (this is critical for byte-exactness):**
- Instruction atoms are created with `align = INST_ALIGN = 2` (`atom.c:462`) — **always** aligned. Auto-alignment emits `general_error(50)` "instruction has been auto-aligned" (WARNING).
- Data-def atoms get `DATA_ALIGN(bitsize)` (`atom.c:501`), but mot **overrides it to 1 unless `align_data`** (`syntax.c:608-609`). `align_data` defaults to 0 and is only set by `-align` or `-devpac` (`syntax.c:2261-2264, 2269-2277`). **So by default `dc.w`/`dc.l` are NOT aligned.**
- Space atoms: `a->align = align_data ? DATA_ALIGN(size) : 1` (`syntax.c:292`).
- `dc.b "str"` uses `new_data_atom(db,1)` — align 1 always.
- Padding for alignment is `sec->pad` = a single `0x00` byte (`vasm.c:1032-1033`), written by `fwpcalign()` (`supp.c:450`). `cnop` in a code section uses `0x4e71` (real NOP) as the fill pattern when `align>3` and not `-devpac` (`syntax.c:690-697`).

Verified:
```
    org 0
    dc.b 1
    dc.w $1234        ; NOT aligned
    dc.b 2
    dc.l $12345678    ; NOT aligned
    dc.b 3
    move.w #1,d0      ; auto-aligned (warning 51 = general_error 50)
    dc.b 4
->  01 12 34 02 12 34 56 78 03 00 30 3c 00 01 04
                                ^^ pad byte
```

**`eval_data()` ranges (`cpu.c:4570-4631`):**
```c
    case 8:
      if (etype == NUM) {
        if (typechk && (val<-0x80 || val>0xff)) cpu_error(39);  /* data out of range */
        db->data[0] = val & 0xff;
      }
      else if (etype == FLT) cpu_error(40);  else ierror(0);
      break;
    case 16:
      if (etype == NUM) {
        if (typechk && (val<-0x8000 || val>0xffff)) cpu_error(39);
        setval(1,db->data,2,val);
      } ...
    case 32:
      if (etype == NUM) setval(1,db->data,4,val);      /* NO range check */
      else if (etype == FLT) conv2ieee32(1,db->data,fval);
      else ierror(0);
      break;
    case 64: /* huge, typechk && !huge_chkrange(hval,64) -> cpu_error(39) */
    case 96: /* ditto, 96 bits */
```
**`dc.b` of a value >255:** `cpu_error(39)` = error **2040** "data out of range" (ERROR), and the byte is still written truncated to `val & 0xff`. Because it is an ERROR, `errors != 0` and `-Fbin` writes **no output file** (`vasm.c:775-783`). Accepted range is `-128 .. 255` inclusive, so both `-1` and `$ff` are fine and both produce `0xff`.
**`dc.w`:** same shape, accepted range `-32768 .. 65535`, `cpu_error(39)` otherwise, truncated to 16 bits.
**`dc.l`:** **no cpu-level range check**; `taddr` is `int32_t` so anything wider is caught by the expression evaluator as general warning 22 "target data type overflow (32 bits)" and wrapped.
Verified: `dc.b 256` → error 2040, `dc.b -129` → error 2040, `dc.w 65536` → error 2040, `dc.l $123456789` → warning 22 only.

---

## 6. Everything else needed to match bytes

**Default CPU.** `static uint32_t cpu_type = m68000;` (`cpu.c:34`). With no `-m` flag you get plain 68000, no FPU, no MMU. `-m68000`/`-m68008` both map to `m68000` (`cpu_models.h:1-2`). `-gas` changes the default to 68020+68881 (`cpu.c:4828-4832`). `set_cpu_type()` (`cpu.c:4716-4735`) replaces the CPU bits and, for 020+/ColdFire, sets `m68k_mid = 2`. `__VASM` is set to `cpu_type & CPUMASK` = 1 for 68000 (`cpu.c:4710`).

**Endianness / value writing.** `setval(1,dest,size,val)` (`supp.c`) writes big-endian, MSB first. `setval_signext(1,d,extsz,valsz,val)` writes `extsz` sign bytes then `valsz` value bytes (used only for float immediates from integer expressions).

**Sign-extension for 16-bit absolute.** `abs.w` accepts `-0x8000 .. 0x7fff` (`cpu.c:4141`) and the 68000 sign-extends it at run time; `opt_abs` uses the identical signed test (`size16[0] = extval>=-0x8000 && extval<=0x7fff`, `cpu.c:1857`). Since `taddr` is `int32_t`, `$fffff000` parses as `-4096` and *does* fit — verified: `move.w $fffff000,d0` → `3038 f000`, while `move.w $8000,d0` → `3039 00008000` (32768 does not fit signed 16). This asymmetry is a common source of mismatches; reproduce it exactly with a signed 32-bit `taddr`.

**Error/warning severity summary (index → number → severity), from `cpu_errors.h`:**

| idx | num | text | severity |
|---|---|---|---|
| 0 | 2001 | instruction not supported on selected architecture | FATAL\|ERROR |
| 1 | 2002 | illegal addressing mode | ERROR |
| 2 | 2003 | invalid register list | ERROR |
| 3 | 2004 | missing ) in register indirect addressing mode | ERROR |
| 4 | 2005 | address register required | ERROR |
| 5 | 2006 | bad size extension | ERROR |
| 6 | 2007 | displacement at bad position | WARNING |
| 10 | 2011 | illegal scale factor | ERROR |
| 15 | 2016 | %c expected | ERROR |
| 16 | 2017 | can't use PC register as index | ERROR |
| 17 | 2018 | double registers in list | WARNING |
| 18 | 2019 | data register required | ERROR |
| 22 | 2023 | need 32 bits to reference a program label | WARNING |
| 24 | 2025 | absolute value expected | ERROR |
| **25** | **2026** | operand value out of range: %ld (valid: %ld..%ld) | ERROR |
| 26 | 2027 | label in operand required | ERROR |
| 27 | 2028 | using signed operand as unsigned … | WARNING |
| 28 | 2029 | branch destination out of range | ERROR |
| **29** | **2030** | **displacement out of range** | ERROR |
| 30 | 2031 | absolute displacement expected | ERROR |
| **32** | **2033** | absolute short address out of range | ERROR |
| 33 | 2034 | deprecated instruction alias | WARNING |
| 34 | 2035 | illegal opcode extension | FATAL\|ERROR |
| 35 | 2036 | extension for unsized instruction ignored | WARNING |
| **36** | **2037** | immediate operand out of range | ERROR |
| 37 | 2038 | immediate operand has illegal type or size | ERROR |
| 38 | 2039 | data objects with %d bits size are not supported | ERROR |
| **39** | **2040** | data out of range | ERROR |
| 40 | 2041 | data has illegal type | ERROR |
| 45 | 2046 | link.w changed to link.l | WARNING |
| 46 | 2047 | branch out of range changed to jmp | WARNING |
| 47 | 2048 | lea-displacement out of range, changed into move/add | WARNING |
| 48 | 2049 | translated (A%d) into (0,A%d) for movep | WARNING |
| 49-54 | 2050-2055 | operand/instruction optimized/translated … | MESSAGE (`-showopt`) |
| 57 | 2058 | short-branch to following instruction turned into a nop | WARNING |
| 60 | 2061 | division by zero | WARNING |

`max_errors` defaults to 5 (`error.c:28`); after that vasm prints "***maximum number of errors reached!***" and exits. Any ERROR suppresses `-Fbin` output entirely.

**`-Fbin` output** (`output_bin.c:24-90`): rejects any remaining `IMPORT` symbol (`output_error(6)`), rejects overlapping sections, sorts sections by `org`, zero-fills gaps between sections, then writes `DATA` atoms verbatim and `SPACE` atoms via `fwsblock`, with `fwpcalign` emitting the section pad pattern (a single `0x00`) before each atom that needs alignment. `-cbm-prg` prepends a 2-byte little-endian load address.

**Instruction-copy plumbing you must model.** `copy_instruction` (`cpu.c:202-224`) makes a *shallow* copy: `expr *` pointers are shared between the copy and the real ip. `free_expr`/`free_operand` calls inside `optimize_instruction` are guarded by `if (final)` precisely because during sizing the expressions still belong to the real ip. In Rust, model an instruction as a value type with `Rc<Expr>` (or an arena index) and only free/replace expressions on the final pass.

**The two-call-per-atom structure.** During `resolve()` each atom's `instruction_size()` runs `optimize_instruction(copy, …, final=0)`; during `assemble()` `eval_instruction()` runs `optimize_instruction(real, …, final=1)` which *permanently* rewrites `ip->code`, `ip->qualifiers[0]`, and the operands. Because `optimize_instruction` is idempotent given the same `pc`/`lastsize`, the final pass reproduces the last sizing pass's decision — but only if `last_size` is exactly what the last sizing pass computed. `eval_instruction` therefore saves/restores `flags` and `last_size` around the whole thing (`cpu.c:4249-4250, 4531-4532`), because `cpu.c:2413` clobbers them via the union. Reproduce this or your final pass will diverge from your sizing pass.

**Known 1.7h quirks to replicate deliberately:**
1. `asl.x Dn` / `lsl.x Dn` (the no-immediate register form) with `opt_gen` produces `ADD.x Dn,D0` — destination register lost (`cpu.c:3052-3053`, verified `d083`).
2. `opt_branop`'s "nop" is `LEA (A6),A6` = `0x4dd6`, not `NOP` = `0x4e71` (`opcodes.h:1244`, verified).
3. `optypes_subset`'s `break` at `cpu.c:2203` is outside its `if`.
4. `val`/`abs` in `optimize_instruction` are read uninitialised when `op[0]==NULL`; initialise to 0 for equivalent behaviour.
5. Operand-level optimisations run before the `no_opt` early-return.
6. `move #0,<ea> → clr` and `andi #0 → clr` do **not** fire on 68000 (need `opt_clr` or 68010+), because `CLR` performs a read-modify-write on the 68000.