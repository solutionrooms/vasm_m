; Object output: code/data/bss hunks, memory flags, xref/xdef, common,
; cross-section pc-relative references, external references of all sizes.
        section code,code
        xdef    start,tab
start:  lea     tab(pc),a0
        move.l  #tab,d0
        move.l  tab,d1
        move.w  tab,d2
        jsr     ext1
        jsr     ext1(pc)
        bsr.w   sub1
        bra.s   .l1
.l1:    bra.w   start
        moveq   #0,d0
        move.l  #ext2,d3
        move.w  #ext2,d4
        move.b  #ext2,d5
        lea     ext2,a1
        rts
sub1:   rts
        section fastdata,data_f
tab:    dc.l    start,ext1,tab+4,sub1
        dc.l    buf,buf+8
        dc.b    1,2,3
        section chipbss,bss_c
buf:    ds.l    4
        comm    cbuf,16
        comm    unrefd,32
        section code
        move.l  #cbuf,d0
        move.l  cbuf,d1
