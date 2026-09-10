; -databss trims whole original atoms, not merged lines or arbitrary bytes.
        section longs,data
        dc.l 1,0,0
        section words,data
        dc.w 1,0,0,0
        section bytes,data
        dc.b 1,0,0,0,0,0
        section strings,data
        dc.b "A",0,0,0,0,0
; Retain the entire nonzero wide operand / string even with zero bytes inside.
        section wide,data
        dc.q $100000000,0
        section opaque,data
        dc.b "A\0\0\0\0\0\0\0",0,0
