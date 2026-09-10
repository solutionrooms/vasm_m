; 16-bit absolute reference to a relocatable label: fine in bin, unsupported
; reloc in hunk (output error 3004 on both assemblers).
        section text,code
start:  nop
        dc.w    start
        move.w  #start,d0
        rts
