; -linedebug: HUNK_DEBUG LINE records per data/space atom.
        section code,code
start:  nop
        move.l  #tab,d0
        dc.b    1,2,3,4
        dc.w    5,6
        ds.w    3
        rts
        section data,data
tab:    dc.l    start
        dcb.b   6,7
