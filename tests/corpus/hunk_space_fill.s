; Space atoms filled with a relocatable expression carry one reloc per element.
        section text,code
start:  nop
        dcb.l   4,start
        dcb.l   2,start+8
        ds.l    2
        blk.w   3,$abcd
        rts
