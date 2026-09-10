<!-- Generated 2026-09-10 by a research agent scanning AssemblyTest/CubeDroid. -->

# vasm 1.7h feature inventory — AssemblyTest/CubeDroid

All paths relative to `AssemblyTest/CubeDroid`. Reference build: `vasmm68k_mot -Fbin -spaces SourceCode/stub.X68` run with CWD = `AssemblyTest/CubeDroid`.

## 0. Build-set determination (read this first)

vasm's include search path (from `vasm-1.7h/vasm.c:527` and `:548`) is exactly two entries, in this order:

1. `.` — the CWD (`AssemblyTest/CubeDroid`)
2. the directory of the **main input file** (`SourceCode/`)

It is **not** the directory of the currently-including file. This is why `SourceCode/CubeDroid/gameplaystate_cubedroid.X68:5033` can say `INCLUDE "CubeDroid/FastStarfield.X68"` and resolve to `SourceCode/CubeDroid/FastStarfield.X68`.

`SourceCode/compiledefs.X68` (included first) decides the conditional includes:
- `IS_GAME_CUBEDROID equ 1` → the CubeDroid branch **is** assembled
- `USE_DUALPCM` is commented out → **`SourceCode/DualPCM/DualPCM.X68` is NOT in the build.** Anything you see only in that file (`dcb.b`, `align $8000`, indented colon-labels, `incbin "DualPCM/Dual PCM - FlexEd.bin"`) is *not* required for byte-exactness.
- `TEST_SOUND` is undefined (only `-DTEST_SOUND=` builds define it), so `ifnd TEST_SOUND` branches win in `SourceCode/memory_map.X68:29`.
- `USE_HVCOUNTER_TIMINGS`, `USE_A6`, `SHOW_BORDER_COLORS`, `DO_PRELOAD`, `USE_PAUSE_MENU`, `IS_INVULNERABLE`, `LEVELS_UNLOCKED`, `CUBEDROID_SKIP_END_SEQUENCE`, `USE_DEBUG_INPUT` all undefined. `IS_FINAL_GAME`, `IS_DEBUG_MODE`, `USE_PSG`, `USE_MUSIC`, `CUBEDROID_SKIP_LANDING_SEQUENCE`, `CD_STARTLEVEL`, `CD_STARTBG` defined.

**Build set: 59 files, 43,940 lines, 1,835,045 bytes of source.**

---

## 1. Directives actually used

Only **23 distinct directives** appear in the build set. Counts are build-set occurrences; spellings are exactly as they appear in source.

| Directive | Count | Case seen | Column | Example |
|---|---|---|---|---|
| `dc.w` | 6730 | lowercase only | 6726 indented / 4 after col-1 label | `SourceCode/stub.X68:94` `dc.w	$0000` |
| `dc.l` | 5546 | lowercase only | 5545 indented / 1 after label | `SourceCode/stub.X68:21` `dc.l	Start` |
| `dc.b` | 3245 | lowercase only | 3044 indented / 201 after label | `SourceCode/stub.X68:88` `dc.b	'SEGA GENESIS    '` |
| `equ` | 1783 | lowercase only | always after a col-1 label | `SourceCode/stub.X68:6` `REG_HWVERSION equ $00A10001` |
| `macro` / `MACRO` | 167 / 5 | both | always after a col-1 label | `SourceCode/stub.X68:11` `BORDER macro \1`; `SourceCode/FADE.X68:23` `FADE_STOP MACRO` |
| `endm` / `ENDM` | 167 / 5 | both | always indented (never col 1) | `SourceCode/stub.X68:15` |
| `so.w` / `so.b` / `so.l` | 229 / 164 / 147 | lowercase only | always after col-1 label | `SourceCode/hvcounter.X68:228` `ghvc_currentTimer   so.w 1` |
| `clrso` | 103 | lowercase | indented, no label | `SourceCode/hvcounter.X68:227` |
| `setso` | 1 | lowercase | indented | `SourceCode/memory_map.X68:45` `setso $ff0000` |
| `rsset` | 1 | lowercase | indented | `SourceCode/memory_map.X68:74` `rsset $ff0000` (RS never used afterwards — dead) |
| `INCBIN` / `incbin` | 152 / 1 | mostly UPPER | indented | `SourceCode/z80.X68:252` `incbin "z80/z80dac.bin"` |
| `INCLUDE` | 59 | **UPPERCASE only** | 51 indented / 8 after col-1 label | `SourceCode/stub.X68:228` `INC2 INCLUDE "ExportedSource/NewFonts.X68"` |
| `align` | 101 | lowercase | indented | `SourceCode/stub.X68:307` `align 2` (also one `align   2`) |
| `endif` | 87 | lowercase | indented | `SourceCode/stub.X68:14` |
| `ifd` | 75 | lowercase | indented | `SourceCode/stub.X68:12` `ifd SHOW_BORDER_COLORS` |
| `rept` / `endr` | 33 / 33 | lowercase | indented | `SourceCode/debugpanel.X68:910` `rept 8` |
| `set` / `SET` | 23 / 4 | both | after col-1 label | `SourceCode/globals.X68:71` `GP   SET $ffFFf000` |
| `ifnd` | 12 | lowercase | indented | `SourceCode/memory_map.X68:29` `ifnd TEST_SOUND` |
| `else` | 9 | lowercase | indented | `SourceCode/hvcounter.X68:126` |
| `cnop` | 3 | lowercase | indented | `SourceCode/ExportedSource/NewSampleGroups.X68:38` `cnop 0,32768`; also `NewSprites.X68:5874` `cnop 0,65536` |
| `end` | 1 | lowercase | indented | `SourceCode/stub.X68:354` `end 0` |

**Never used anywhere in the tree:** `section`, `even`, `ds`/`ds.b/w/l`, `blk`, `dr`, `opt`, `xdef`, `xref`, `public`, `global`, `comm`, `offset`, `printt`, `printv`, `echo`, `fail`, `idnt`, `list`/`nolist`, `page`, `output`, `symdebug`, `cargs`, `inline`, `auto`, `endc`, `elseif`, `ifc/ifnc/ifb/ifnb/ifeq/ifne/iflt/ifgt/ifle/ifge`, `rs`/`rsreset`, `fo`/`clrfo`/`setfo`, `mexit`, `rem/erem`, `incdir`.

**Used elsewhere in the tree but not in the build:** `org` (5×, only in `RealtimeData/stub_compile_*.X68`), `dcb.b` (4× in `SourceCode/DualPCM/DualPCM.X68`, `SourceCode/Sound/Sound_DualPCM_Data.X68`, `SourceCode/ExportedSource/exportedsamplemusic.X68`).

**Column rule (holds without exception):** no directive and no mnemonic ever starts at column 1. Column 1 is *only* ever a label. Conversely `endm`, `endif`, `endr`, `else`, `clrso` are always indented.

### The critical structural quirk: two statements on one line

vasm's `parse()` (`syntax/mot/syntax.c:1730-1790`) handles `label EQU expr` and then **falls through** to directive/mnemonic parsing on the rest of the line. `SourceCode/globals.X68` exploits this **222 times**:

```
sysg_vblcount equ GP            GP_L 1
sysg_music equ GP 	            GP_B sgmusic_structsize
```

That is `sysg_vblcount = GP`, then a *separate* macro invocation `GP_L 1` which does `GP SET GP+(1*4)`. Breakdown: `equ`+`GP_W` ×103, `equ`+`GP_L` ×99, `equ`+`GP_B` ×20 — all in `SourceCode/globals.X68:87-380`. **If your assembler stops parsing a line after EQU, every RAM symbol after the first will be wrong.**

---

## 2. Macro features

- **157 distinct macro names** defined across the tree; **172 `macro` directives** in the build set (17 names are defined twice — see below).
- **Parameter references used: `\1`(255), `\2`(117), `\3`(70), `\4`(41), `\5`(9), `\6`(5) only.** Max index 6.
- **`\@` — never used. `\0` — never used. `\#` — never used. `\<sym>` — never used. Named/keyword params — never used. Default args — never used. `narg`/`NARG` — never used. `CARG` — never used. `REPTN` — never used. `mexit` — never used.**
- The only other backslash in any source is `\S` inside a Windows path string: `RealtimeData/stub_compile_music_binary.X68:5` `INCLUDE "..\SourceCode\Sound\Sound_MusicDefs.X68"` (not in the ROM build). Escape sequences are **off** by default in mot syntax (`syntax.c:2272`), so `\S` stays literal.
- **Definition-line operands are decorative and MUST be ignored.** `BORDER macro \1`, `STATE_SET_CALLBACKS macro \1 \2`, `MEM_ALLOC_ITEM    macro   \1,\2`, and even the typo `FONT_DRAW_AT_A0 macro \1 \3 \3` (`SourceCode/globals.X68:479`). vasm calls `new_macro(labname,endm_dirlist,NULL)` and `continue`s — nothing after `macro` is parsed. Both comma-separated and space-separated forms appear.
- **Macro name may carry a colon:** `SourceCode/debug.X68:22` `DEBUG_SHOWMEM_A0: macro` and `:25` `DEBUG_SHOWMEM: macro`. vasm re-reads the raw label field via `parse_identifier`, so the colon is stripped.
- **Invocation case always matches definition case exactly** — 0 mismatches found. (`GOManager_GetList`, `SPRCACHE_Declare`, `MUSIC_SETREGA_TRACK_A0_NOd5` etc. keep their mixed case.) You can implement case-sensitive macro lookup safely, but case-insensitive would also work here.
- **No macro is ever invoked with a `.size` suffix** — 0 occurrences. Qualifier/`\0` handling is not exercised.
- **Nested macro *definition*** occurs once, in `SourceCode/scroll.X68:519` — **not in the build set**. You do not need it.
- **Macro calling another macro** (in the build set, 3 cases): `SourceCode/SpriteCache.X68:190` `SPRCACHE2_SETBASE` → `PUSHREG`; `SourceCode/gameplay_main.X68:3` `STATE_GENERIC_INIT` → `AUTOINC`. Max recursion depth 2.
- **Conditionals inside macro bodies** (11 macros): e.g. `SourceCode/globals.X68:54` `PUSHALL` uses `ifd USE_A6 / else / endif`; `SourceCode/stub.X68:11` `BORDER` uses `ifd SHOW_BORDER_COLORS`.
- **`rept` inside a macro body, with a macro parameter as the count**: `SourceCode/SpriteCache.X68:9` `SPRCACHE_DeclareMulti` contains `rept \2`. Not invoked in the build set.
- **Local labels inside macro bodies (9 macros)** — e.g. `SourceCode/debug.X68:11` `.debugmessage dc.b \1,0` inside `DEBUG_SHOWMESSAGE`, and `SourceCode/gameobject_data.X68:5` `SET_COMPSPR_COLLIDER` with `.xflip:`/`.exit:`. These rely on vasm's "local label belongs to the enclosing global label" scoping; each invocation site sits under a different global label. No `\@` uniquing is used.
- **Duplicate macro names** (17, all resolved by conditionals): `HVTIMER_START0..7`/`HVTIMER_END0..7` are defined at `SourceCode/hvcounter.X68:6-119` inside `ifd USE_HVCOUNTER_TIMINGS` and again as empty stubs at `:128-158` in the `else`. Only the empty ones reach the build. `BORDER` is defined in both `stub.X68` and `stub_musicplayer.X68` (different translation units).
- **Macro arguments can be register lists:** `SourceCode/globals.X68:34` `PUSHREGS_W macro \1` → body `movem.w \1,-(sp)`, invoked as e.g. `PUSHREGS d0-d7/a0-a2`.
- Macro invocations (build set): **82 distinct macros invoked, 955 invocation sites.** Top: `VRAM_DECLARE_CHARPOS_A`(100), `POPALL`(87), `PUSHALL`(86), `SPRCACHE_Declare`(73), `MUSIC_PSG_PITCH`(61), `FONT_DRAW_AT`(42), `MUS_REST`(39), `SETSPR`(33).

---

## 3. Expression features

**Number / literal formats used:**

| Form | Count | Note |
|---|---|---|
| decimal | ~128,266 | plain |
| `$hex` | ~72,463 | `SourceCode/stub.X68:6` `equ $00A10001` |
| `'…'` single-quoted | 856 | both char constants and strings; lengths 0–48 chars |
| `"…"` double-quoted | 230 | **only** ever as `INCLUDE`/`INCBIN` filenames — never as data |
| `%binary` | 21 | `SourceCode/ExceptionHandler.X68:1027` `dc.w %0100111001110000` |
| `@octal` | **0** | never used |

Multi-char immediates used: `#'SEGA'`(2), `#'HEAP'`, `#'NORM'`, `#'M__C'`, `#'M__L'`, `#'M__R'`, `#'0'`(7), `#' '`(3), `#'G'`. Example: `SourceCode/stub.X68:117` `move.l	#'SEGA',REG_TMS`. Zero-length `''` occurs once.

**Operators used (build set):**

| Op | Count | Example |
|---|---|---|
| unary `-` | ~7,022 | `SourceCode/datatables.X68:210` `dc.w -181,-181,…` |
| `+` | ~3,171 | `SourceCode/hvcounter.X68:8` `lea HVCounterRam+(0*hvc_structsize),a0` |
| `-` binary | frequent | `SourceCode/stub.X68:245` `INCSIZE_GLOBAL_PALETTES equ INC1-INC0` |
| `\|` (bitwise or) | 976 | `SourceCode/stub.X68:13` `move.w #REG_BACKGROUND_COLOR\|\1,VDP_CONTROL` |
| `*` | 244 | `SourceCode/memory_map.X68:16` `equ 96 * 8` |
| `<<` | 86 | `SourceCode/globals.X68:440` `padmask_button_up equ 1<<pad_button_up` |
| `>>` | 34 | `SourceCode/stub.X68:321` `dc.b ($C000>>10)` |
| `&` | 16 | `SourceCode/vdp.X68:147` `#REG_PATTERNNAME_SCROLLA \| ((\1&$e000)>>10)` |
| `~` | 1 | `SourceCode/CubeDroid/gameplaystate_cubedroid.X68:4189` `and.b #~\2,17(a2,d1)` |
| `/` as divide | **0** (all `/` are movem separators) | |
| `%` as modulo | **0** (all `%` are binary literals) | |
| `^`, `!`, `&&`, `\|\|`, `==`, `!=`, `<`, `>`, `<=`, `>=` | **0** | never used |

**Parentheses:** used heavily for grouping and for addressing modes. Nesting up to 3 deep (`SourceCode/lz4w.X68:22` `((.jump_table-.jump_base)-2)(pc,d0.w)`).

**`*` as current PC:** used, but only inside macro bodies that are never invoked in the ROM build — `SourceCode/debug.X68:4` `beq *+4` (inside `DEBUG_SHOWREGS_ONBUTTON`, 0 invocations) and `SourceCode/scroll.X68:551`, `SourceCode/Sound/Sound_DualPCM.X68` (files not in the build). Safe to implement but not load-bearing.

**Label arithmetic in data and equates** is load-bearing:
- `SourceCode/ExportedSource/NewPSGEffects.X68` — 21 `dc.l Label-.a` (global minus *local* label).
- `SourceCode/stub.X68:244-256` — 13 `INCSIZE_* equ INCn-INCm` where `INCn` are bare column-1 labels placed between `INCLUDE` lines.
- `SourceCode/globals.X68:382-384`, `SourceCode/datatables.X68:638` — `equ Label_End-Label_Start`.

---

## 4. Label conventions

| Form | Count (build set) | Notes |
|---|---|---|
| Column-1 label **with** trailing colon | 3,571 | `SourceCode/stub.X68:107` `Start:` |
| Column-1 label **without** colon | 2,867 | `SourceCode/stub.X68:6` `REG_HWVERSION equ …`, `SourceCode/stub.X68:239` `INC12 INCLUDE …` |
| Indented label (colon **mandatory**) | 7 | `SourceCode/CubeDroid/gameplaystate_cubedroid.X68:4642` ` .doOne:`, `:5005` `  .yvels:`, `:4927` ` CD_GO_EndSeqPlayer: ` |
| Local labels (leading `.`) at column 1 | 3,358 | `SourceCode/stub.X68:112` `.checksecurity:` |
| Local labels indented with colon | 7 | (subset of the row above) |
| Column-1 mnemonic or directive | **0** | never happens |

`parse_labeldef()` (`vasm-1.7h/parse.c:305`): colon is optional at column 1, **mandatory** when indented. Note `SourceCode/stub.X68:226-240` uses bare column-1 labels whose only purpose is address capture, several followed immediately by a `;` comment:
```
INC0 ;INCLUDE "ExportedSource/globalpalettes.X68"
INC13
```
`\@` labels: **never used**.

Identifier charset: `ISIDSTART` = `. @ _ alpha`; `isidchar` also allows `$` and `%`. No identifier in this codebase actually contains `$` or `%`, and none starts with `@`. `dot_idchar` is **off** (not `-ldots`/`-phxass`), so a trailing `.w`/`.l` on an identifier is a size suffix, not part of the name — this is exactly what `SourceCode/DMAList.X68:29` `lea g_dmaList.w,a0` relies on (52 such short-absolute operands).

Labels sharing a line with a directive: `equ`×1783, `macro`×172, `so.b/w/l`×540, `dc.b/w/l`×206, `set`×27, `INCLUDE`×8.

---

## 5. Instruction set

**59 distinct mnemonics**, all lowercase (zero uppercase/mixed-case mnemonic spellings). 68000-only — no 010/020+, no FPU, no MMU, no CPU32.

```
abcd  add   addi  addq  addx  and   andi  asl   asr   bcc   bclr  bcs   beq
bge   bgt   ble   bls   blt   bmi   bne   bpl   bra   bset  bsr   btst  clr
cmp   dbf   dbra  divs  divu  eor   exg   ext   jmp   jsr   lea   lsl   lsr
move  movem movep moveq muls  mulu  neg   nop   not   or    rol   ror   rte
rts   sne   sub   subq  swap  trap  tst
```

Size-suffix usage per mnemonic (`(none)` = unsized, assembler picks default — usually `.w`):

| Mnemonic | total | sizes |
|---|---|---|
| `move` | 5337 | none:1900, b:467, w:900, l:2070 |
| `bsr` | 965 | none:964, b:1 |
| `rts` | 907 | none:907 |
| `add` | 832 | none:596, b:12, w:94, l:130 |
| `lea` | 708 | none:708 |
| `jsr` | 667 | none:667 |
| `bra` | 368 | none:357, s:10, w:1 |
| `beq` | 347 | none:343, b:1, s:3 |
| `cmp` | 332 | none:224, b:62, w:10, l:36 |
| `lsl` | 253 | none:229, b:1, l:23 |
| `and` | 246 | none:171, b:19, w:19, l:37 |
| `moveq` | 228 | none:228 |
| `sub` | 198 | none:135, b:5, w:19, l:39 |
| `dbra` | 143 | none:143 |
| `bne` | 140 | none:139, s:1 |
| `lsr` | 137 | none:91, b:1, w:19, l:26 |
| `tst` | 102 | none:51, b:41, w:6, l:4 |
| `or` | 97 | none:25, b:26, w:39, l:7 |
| `jmp` | 84 | none:84 |
| `movem` | 83 | w:12, l:71 |
| `btst` | 74 | none:70, b:4 |
| `blt` | 76 | none:76 · `bgt` 61 none · `bge` 49 none · `neg` 53 (none:20,w:30,l:3) · `mulu` 50 (none) · `swap` 45 (none) · `subq` 46 (none:44,w:1,l:1) · `addq` 34 (none:25,b:2,w:4,l:3) · `asr` 26 (none:9,w:1,l:16) · `ble` 23 none · `bmi` 20 (none:19,b:1) · `rol` 19 (none:11,b:1,w:4,l:3) · `divu` 16 none · `ext` 14 (w:2,l:12) · `nop` 14 none · `muls` 13 none · `clr` 9 (w:7,l:2) · `bclr` 8 (none:5,b:3) · `movep` 8 (w:4,l:4) · `trap` 8 none · `eor` 7 (none:1,b:4,l:2) · `asl` 7 (none:3,l:4) · `ror` 7 (none:3,b:3,l:1) · `abcd` 6 none · `bset` 5 none · `exg` 5 (l:5) · `dbf` 4 none · `sne` 4 none · `addx` 4 (w:2,l:2) · `bcs` 4 none · `divs` 3 none · `rte` 3 none · `bpl` 3 (none:1,b:1,s:1) · `andi` 3 (b/w/l:1 each) · `bcc` 2 (s:2) · `not` 2 (none:1,b:1) · `bls` 2 (none:1,s:1) · `addi` 1 (l) | |

**Both `.b` and `.s` branch suffixes appear** (`.s`×26, `.b`×5) — e.g. `SourceCode/stub.X68:116` `beq.b .skip` and `SourceCode/ExceptionHandler.X68:2913` `bra.s .lp`. `exg.l` is written with an explicit `.l` (5×) which vasm accepts.

**Addressing-mode syntax forms used:**

| Form | Count | Example |
|---|---|---|
| `#imm` | 3,361 | `SourceCode/stub.X68:290` `moveq	#$17,d1` |
| `Dn` / `An` | 5,685 / 1,604 | |
| `(An)+` / `(sp)+` | 2,614 / 144 | `SourceCode/stub.X68:288` `movem.w	(a5)+,d5-d7` |
| `d16(An)` | 2,139 | `SourceCode/hvcounter.X68:165` `move HVCOUNTER,hvc_starttime(a0)` |
| `(An)` | 276 | `SourceCode/stub.X68:147` `jsr (a0)` |
| `-(An)` / `-(sp)` | 33 / 126 | `SourceCode/hvcounter.X68:7` `movem.l a0/d0,-(sp)` |
| **`d8(An,Xn)` with NO index size** | 105 | `SourceCode/debugpanel.X68:908` `move.l (a0,d0),a0` — must default to `.w` |
| `d8(An,Xn.w)` | 38 | `SourceCode/FADE.X68:326` `move (a0,a3.w),(a1)+` — **address register as index** |
| **`d8(PC,Xn)` with NO index size** | 34 | `SourceCode/ExceptionHandler.X68:306` `move .pals(pc,d0),d5` |
| `d8(PC,Xn.w)` | 5 | `SourceCode/lz4w.X68:22` `move.l  ((.jump_table-.jump_base)-2)(pc,d0.w),a4` |
| `d16(PC)` | 1 | `SourceCode/stub.X68:287` `lea	SetupValues(pc),a5` |
| `sym.w` (short absolute) | 52 | `SourceCode/DMAList.X68:29` `lea g_dmaList.w,a0` |
| `(expr).w` (short absolute) | 34 | `SourceCode/debugpanel.X68:88` `move #\1,(sysg_debugpanel+debugpanel_x_cursor).w` |
| `movem` register lists | 55 | see below |
| `sr` | 5 | `SourceCode/stub.X68:110` `move.w	#$2700,sr` |
| `ccr` | 2 | `SourceCode/text.X68:292` `and.b   #$EF, ccr` |
| `sp` as `a7` alias | 11+ | `SourceCode/DMAList.X68:109` `lea     -10(sp),sp` |

**Not used:** `(d16,An)` / `(d8,An,Xn)` / `(d16,PC)` "new" bracketed syntax — **zero occurrences**; only the classic `d(An,Xn)` form. Also unused: `usp`, `vbr`, `sfc`, `dfc`, scale factors (`*2`/`*4`/`*8`), memory-indirect `([…])`, `abs.l` explicit suffix, `movec`, bitfields.

**movem register-list forms seen** (all classic `-`/`/` syntax):
```
a0/d0            d7/a0            d2-d3            d5-d7            a0-a4
d0-d7/a0-a5      d0-d7/a0-a6      d0-d7/a1-a6      d0-d7/a1-a4      d0-d7/a2-a5
d0-d5/a1         d0-d5/a0-a4      d0-d5            d1-d7            d1-d4
a0-a2            a0-a2/a4/d0-d7   d7/a1/a2/a5      d0-d7/a3-a6      d0-d3
\1  (macro parameter)
```
Note `a0-a2/a4/d0-d7` and `d7/a1/a2/a5` — address registers listed **before** data registers, and singleton registers mixed with ranges.

---

## 6. `-spaces`-sensitive constructs

Under `-spaces` (`allow_spaces=1`, `syntax.c:2287`): `exp_skip()` no longer terminates an operand at whitespace; `skip_operand()` runs to a top-level comma, a `;`, or EOL; `eol()` then requires true end-of-line and emits *warning* 6 ("garbage at end of line") otherwise. `iscomment()` returns true **only for `;`** (the `*`-after-blank rule is `-phxass` only). `check_comm` warning 18 is suppressed.

I scanned every operand in the build set for internal whitespace. Results:

**Genuine `-spaces`-dependent lines (would break without `-spaces`):**

| Kind | Count | Examples |
|---|---|---|
| spaces around `+` inside an operand | 23 | `SourceCode/globals.X68:460` `move.l #\1 + (\4*128) + (\3*2),d1` |
| spaces around `\|` inside an operand | 19 | `SourceCode/vdp.X68:144` `move #REG_HSCROLL \| (\1>>10),VDP_CONTROL`; `SourceCode/vdp.X68:147` `move.w	#REG_PATTERNNAME_SCROLLA \| ((\1&$e000)>>10), VDP_CONTROL` |
| **spaces around `/` inside a movem register list** | 2 | `SourceCode/IMAGE.X68:161` `movem.l a0-a1 / d0-d1,-(sp)` and `SourceCode/IMAGE.X68:178` `movem.l (sp)+,a0-a1 / d0-d1` |
| spaces around `*` inside an operand | 1 | `SourceCode/Sound/Sound_PSG.X68:38` `move #(MAX_PSGSFX * psgsfx_structsize)-1,d0` |
| the 222 `equ GP  GP_L 1` two-statement lines (§1) | 222 | `SourceCode/globals.X68:87` |

Plus space after comma between operands, which is pervasive and harmless: `SourceCode/text.X68:79` `addq.l  #1, a0`, `SourceCode/text.X68:293` `abcd    -(a2), -(a3)`, `SourceCode/stub.X68:101` `dc.l	$200001, $20ffff`.

**Comments:** every inline comment in the build set is introduced by `;`. There are **zero** cases of an operand followed by undelimited comment text — I found no "garbage at end of line" candidates once operator-spacing and macro-definition lines are excluded. Full-line comments: 1,579 starting with `;` at column 1, and **3** starting with `*` at column 1 (`SourceCode/text.X68:336`, `:337`, `SourceCode/utils.X68:113` `***…***`). A `*` at column 1 *is* a comment (handled in `parse()` before mnemonic lookup) but `*` mid-line is never a comment here.

A stylistic pattern that looks like a C comment but is just a `;` comment: `SourceCode/stub.X68` has 33 lines of the form `dc.b		$04			;/* VDP $80 - 8-colour mode*/`.

**Include filename with an embedded space:** `SourceCode/Sound/Sound.X68:3` `INCLUDE "Sound/Sound_Music _PSG.X68"` — the file really is named `Sound_Music _PSG.X68`. The quotes make this work; your filename parser must not split on whitespace inside quotes.

---

## 7. Include tree and other translation units

### 7a. `SourceCode/stub.X68` include tree (build order, 59 files, 43,940 lines, 1,835,045 bytes)

```
SourceCode/stub.X68
├─ SourceCode/compiledefs.X68
├─ SourceCode/hvcounter.X68
├─ SourceCode/debug.X68
├─ SourceCode/globals.X68
├─ SourceCode/DMAList.X68
├─ SourceCode/vdp.X68
├─ SourceCode/memory_map.X68
├─ SourceCode/debugpanel.X68
├─ SourceCode/ExceptionHandler.X68
├─ SourceCode/GOManager.X68
├─ SourceCode/SpriteCache.X68
├─ SourceCode/Structs.X68
├─ SourceCode/Mem.X68
├─ SourceCode/gameobject_data.X68
├─ SourceCode/gameobject_structs.X68
├─ SourceCode/FADE.X68
├─ SourceCode/sram.X68
├─ SourceCode/gameplay_main.X68
├─ SourceCode/CubeDroid/gameplaystate_cubedroid.X68        [ifd IS_GAME_CUBEDROID → taken]
│   ├─ SourceCode/CubeDroid/FastStarfield.X68
│   ├─ SourceCode/CubeDroid/gameplaystate_cubedroid_titleScreen.X68
│   ├─ SourceCode/CubeDroid/gameplaystate_cubedroid_levelselect.X68
│   ├─ SourceCode/CubeDroid/gameplaystate_cubedroid_cutscene.X68
│   ├─ SourceCode/CubeDroid/Cubedroid_Cutscene0.X68
│   ├─ SourceCode/CubeDroid/state_clearSavecubedroid.X68
│   ├─ SourceCode/CubeDroid/state_creditsCubedroid.X68
│   ├─ SourceCode/CubeDroid/state_helpCubedroid.X68
│   └─ SourceCode/CubeDroid/state_preload.X68
├─ SourceCode/ExportedSource/CubeDroid.X68                 [same ifd block]
├─ SourceCode/GOTOOLS.X68
├─ SourceCode/utils.X68
├─ SourceCode/Compression.X68
├─ SourceCode/lz4w.X68
├─ SourceCode/game_realtimeCommands.X68
├─ SourceCode/sprite_generic.X68
├─ SourceCode/text.X68
├─ SourceCode/Sound/Sound.X68
│   ├─ SourceCode/Sound/Sound_MusicDefs.X68
│   ├─ SourceCode/Sound/Sound_Music.X68
│   ├─ SourceCode/Sound/Sound_Music _PSG.X68      ← space in filename
│   ├─ SourceCode/Sound/Sound_Music_FMModifier.X68
│   ├─ SourceCode/Sound/Sound_PSG.X68
│   ├─ SourceCode/Sound/Sound_Samples.X68
│   ├─ SourceCode/Sound/Sound_SampleTrack.X68
│   └─ SourceCode/Sound/Sound_Music_RealtimeCommands.X68
├─ SourceCode/Joystick.X68
├─ SourceCode/IMAGE.X68
├─ SourceCode/font.X68
├─ SourceCode/z80.X68
├─ (SourceCode/DualPCM/DualPCM.X68)   ← SKIPPED: ifd USE_DUALPCM is false
├─ SourceCode/datatables.X68
├─ SourceCode/ExportedSource/NewFonts.X68          (label INC2)
├─ SourceCode/ExportedSource/NewImages.X68         (label INC3)
├─ SourceCode/ExportedSource/NewCompoundSprites.X68(label INC4)
├─ SourceCode/ExportedSource/NewSampleGroups.X68   (label INC8)
├─ SourceCode/ExportedSource/NewPSGEffects.X68     (label INC9)
├─ SourceCode/ExportedSource/NewSprites.X68        (label INC10)
├─ SourceCode/ExportedSource/NewMusic.X68          (label INC11)
└─ SourceCode/ExportedSource/NewLevels.X68         (label INC12)
```

No file is included twice. Max include depth 2. `SourceCode/stub.X68:226-236` also contains commented-out `INC0/INC1/INC5/INC6/INC7` includes whose labels are still live and used in `INCSIZE_*` subtraction — so `INCSIZE_GLOBAL_PALETTES`, `INCSIZE_COMPSPRS` etc. evaluate to 0 / to the wrong span by design. Don't "fix" that.

Largest files: `ExportedSource/NewSprites.X68` (7,988 lines), `CubeDroid/gameplaystate_cubedroid.X68` (5,041), `ExceptionHandler.X68` (2,917), `lz4w.X68` (2,914).

### 7b. `RealtimeData/stub_compile_*.X68` — yes, separate translation units

Five independent builds, each run with CWD = `RealtimeData`:

```
RealtimeData/CompileMapsBinary.bat:
  ..\Assemblers\vasmm68k_mot -o MapsBinary.bin -Fbin -spaces stub_compile_maps_binary.X68
RealtimeData/CompileMusicBinary.bat:
  ..\Assemblers\vasmm68k_mot -o MusicBinary.bin -Fbin -spaces stub_compile_music_binary.X68
RealtimeData/CompilePathsBinary.bat:
  ..\Assemblers\vasmm68k_mot -o PathsBinary.bin -Fbin -spaces stub_compile_paths_binary.X68
RealtimeData/CompilePSGEffectsBinary.bat:
  ..\Assemblers\vasmm68k_mot -o PSGEffectsBinary.bin -Fbin -spaces stub_compile_psgeffects_binary.X68
  exit
RealtimeData/CompileSpawnersBinary.bat:
  ..\Assemblers\vasmm68k_mot -o SpawnersBinary.bin -Fbin -spaces stub_compile_spawners_binary.X68
```

Each stub is 3–8 lines: `org $400002`, a few `equ`s, then one or two `INCLUDE`s of `MapsExport.X68` / `MusicExport.X68` / `PathsExport.X68` / `PSGEffectsExport.X68` / `SpawnersExport.X68`. `stub_compile_music_binary.X68:5` additionally does `INCLUDE "..\SourceCode\Sound\Sound_MusicDefs.X68"` with **backslash separators** — vasm's `convert_path()` handles this on Windows; on a POSIX host it will not. This is the **only** use of `org` in the whole tree.

`CubeDroid/CompileMusicBinary.bat` at the top level references `stub_compile_music_binary.X68` from the wrong directory and is stale/broken.

---

## 8. `incbin` binaries

**153 `incbin` directives in the build set, 540,887 bytes total.**

| Source file | files | bytes |
|---|---|---|
| `SourceCode/ExportedSource/NewImages.X68` (lines 383–1573) | 150 | 535,472 |
| `SourceCode/ExportedSource/NewLevels.X68:136,141` | 2 | 5,312 |
| `SourceCode/z80.X68:252` | 1 | 103 |

All paths are quoted and relative to the CWD (`AssemblyTest/CubeDroid`): `"ExportedSource/NEWIMAGEGROUP_IMAGEDEF_*.bin"`, `"ExportedSource/Level_0_TileMap_BG.bin"` (4096 B), `"ExportedSource/Level_0_TileData_BG.bin"` (1216 B), `"z80/z80dac.bin"` (103 B — produced by `CompileZ80.bat` → `Assemblers/tniasm.exe SourceCode/z80/z80dac.s`, so it must exist before the vasm run).

**No `incbin` anywhere uses the optional offset/length arguments** — filename only. Sizes range from 8 bytes (`…_BG0_OBSCURED_MapData.bin`) to 9,728 bytes (`…_PLACEHOLDER_CREDITSSCREEN_CellData.bin`).

Not in the build: `SourceCode/DualPCM/DualPCM.X68:44` `incbin "DualPCM/Dual PCM - FlexEd.bin"` (space in filename), `SourceCode/ExportedSource/images_musicplayer.X68` (3,840 B), `SourceCode/ExportedSource/exportedsamplemusic.X68:19` (target file missing).

---

## 9. Unusual things worth flagging

1. **No `ORG`, no `SECTION`, no `OFFSET` in the ROM build.** Everything lands in one implicit section starting at 0. `-Fbin` writes it flat. The ROM header at `SourceCode/stub.X68:88-104` therefore depends on the vector table above it being exactly 256 bytes.
2. **`end 0`** at `SourceCode/stub.X68:354` — the `end` directive with an (ignored) operand. Sets `parse_end`; anything after it is skipped.
3. **Structure offsets use `SO`/`CLRSO`/`SETSO`, not `RS`.** 540 `label so.b/w/l N` lines plus 103 `clrso` and one `setso $ff0000`. A single stray `rsset $ff0000` at `SourceCode/memory_map.X68:74` sets the RS counter which is then never read. Note `align_data` is **off**, so `so` does not auto-align — `so.w`/`so.l` after an odd `so.b` will produce odd offsets. That matters for byte-exactness.
4. **`cnop 0,32768` and `cnop 0,65536`** at `SourceCode/ExportedSource/NewSampleGroups.X68:38` and `SourceCode/ExportedSource/NewSprites.X68:5874,6872` — 32 KB / 64 KB alignment, injecting large padding blocks into the ROM. Get the fill byte and the "already aligned → no padding" edge case right.
5. **Enormous lines.** Longest is `SourceCode/datatables.X68:210` at **34,950 characters with 8,192 comma-separated `dc.w` operands** (mostly negative decimals). 9 lines exceed 4,096 chars; 18 more are 1,025–4,096; 487 are 256–1,024. vasm has no line-length cap (it mmaps the file and works on pointers) — do not use a fixed line buffer. Note also m68k's `MAX_OPERANDS` is 6, but that limit applies to *instructions* only; `dc` uses an unbounded loop.
6. **Mixed line endings.** 50 of the 59 build files are CRLF, 9 are LF-only (the machine-generated `ExportedSource/New*.X68` mostly). 21,017 lines contain tabs; indentation is inconsistently tabs, 4 spaces, or 1 space (`SourceCode/scroll.X68:7` ` clrso`). Trailing whitespace after operands is common (`SourceCode/hvcounter.X68:233` `hvc_starttime   so.w 1 `).
7. **One non-ASCII file:** `SourceCode/SpriteBitmap.X68` contains bytes ≥ 0x80 — not in the build set.
8. **`dc.b` mixing strings and numbers on one line:** `SourceCode/stub.X68:99` `dc.b 	'RA',$f8,$20`; `SourceCode/CubeDroid/gameplaystate_cubedroid_titleScreen.X68:179` `dc.b	'TEST SOUND FX',ETX`. Strings are `'…'` only; `"…"` is never data.
9. **No string escapes anywhere in data.** `esc_sequences` is 0 by default in mot syntax, and the only backslash inside quotes in the whole tree is the Windows include path in a non-ROM stub.
10. `INC13` at `SourceCode/stub.X68:240` is a bare label on an otherwise empty line, used only as the terminator of the `INCSIZE_LEVELS equ INC13-INC12` computation.
11. `SourceCode/stub.X68:90` has a leading space before the tab: ` 	dc.b	'2026.DEC'` — still indented, still a directive, fine.

---

## Every vasm invocation in the AssemblyTest tree

| Script | Command line |
|---|---|
| `compile.bat` (**the reference build**) | `"Assemblers\vasmm68k_mot" -o Output/%ROM_NAME% -Fbin -spaces SourceCode/stub.X68` (then `..\Tools\SnakeRomPadder.exe Output/CubeDroid.bin`) |
| `run_linux.sh`, `run_linux_first.sh` | `../../vbcc/bin/vasmm68k_mot -Fbin -spaces -L GOP.txt SourceCode/stub.X68 -o GOP.bin` |
| `run_musictest.bat` | `Assemblers/vasmm68k_mot -o Output/MusicPlayer.bin -Fbin -spaces -DTEST_SOUND=0 SourceCode/stub_musicplayer.X68` |
| `RunMusicPlayerBlastem.bat` | `Assemblers\vasmm68k_mot -o Output/MusicPlayer.bin -Fbin -spaces -DTEST_SOUND=0 SourceCode/stub_musicplayer.X68` |
| `RunMameDebugMusicTest.bat` | `Assemblers\vasmm68k_mot -o Output/MusicPlayer.bin -Fbin -spaces -DTEST_SOUND=%1 SourceCode/stub_musicplayer.X68` |
| `run_musicplayer_linux.sh` | `../../vbcc/bin/vasmm68k_mot -o MusicPlayer.bin -Fbin -spaces -DTEST_SOUND=%1 SourceCode/stub_musicplayer.X68` |
| `CompileMusicBinary.bat` (stale) | `Assemblers/vasmm68k_mot -o C:/OnlineRepositories/Megadrive/ROMS/MusicBinary.bin -Fbin -spaces stub_compile_music_binary.X68` |
| `RealtimeData/CompileMapsBinary.bat` | `..\Assemblers\vasmm68k_mot -o MapsBinary.bin -Fbin -spaces stub_compile_maps_binary.X68` |
| `RealtimeData/CompileMusicBinary.bat` | `..\Assemblers\vasmm68k_mot -o MusicBinary.bin -Fbin -spaces stub_compile_music_binary.X68` |
| `RealtimeData/CompilePathsBinary.bat` | `..\Assemblers\vasmm68k_mot -o PathsBinary.bin -Fbin -spaces stub_compile_paths_binary.X68` |
| `RealtimeData/CompilePSGEffectsBinary.bat` | `..\Assemblers\vasmm68k_mot -o PSGEffectsBinary.bin -Fbin -spaces stub_compile_psgeffects_binary.X68` |
| `RealtimeData/CompileSpawnersBinary.bat` | `..\Assemblers\vasmm68k_mot -o SpawnersBinary.bin -Fbin -spaces stub_compile_spawners_binary.X68` |

**The complete flag set you must support: `-Fbin`, `-spaces`, `-o <file>`, `-DSYM=value`, `-L <file>` (listing).** Nothing else. No `-I`, no `-quiet`, no `-nocase`, no `-ldots`, no `-align`, no `-phxass`, no `-devpac`, no `-m68020`.

Non-vasm assemblers in the pipeline: `CompileZ80.bat` → `"Assemblers/tniasm.exe" SourceCode/z80/z80dac.s` (produces the incbin'd `z80/z80dac.bin`), and `CompileMegaPCM.bat` → `Assemblers/tniasm SourceCode/MegaPCM/MegaPCM.s` (not part of the ROM build). `Output/CubeDroid.bin` is 1,048,576 bytes after `SnakeRomPadder.exe` pads it, so compare your vasm output *before* padding.