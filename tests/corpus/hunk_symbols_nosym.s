; Symbol hunks: exported equates (EXT_ABS), exported labels (EXT_DEF), local
; labels excluded, labels derived from equates, weak symbol converted to global.
        section text,code
CONST   equ     $1234
        xdef    CONST,entry
entry:  nop
.loop:  bra.s   .loop
        weak    weakfn
weakfn: rts
alias   equ     entry+2
        xdef    alias
after:  dc.l    alias
        section data,data
d1:     dc.l    entry,after
