; Explicit instruction sizes keep this independent of optional optimizations.
        org     0
start:
        moveq   #42,d0
        addq.l  #1,d0
        bra.s   done
        nop
done:
        rts
        dc.w    $1234
        dc.l    $89abcdef
