; opt x+ : only xdef symbols in the object (hunk_onlyglobal), no HUNK_SYMBOL.
        opt     x+
        section text,code
        xdef    entry
entry:  nop
hidden: rts
        dc.l    hidden
