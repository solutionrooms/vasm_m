; Absolute (org) section next to a relocatable one: org labels need no relocs
; and become absolute symbols; the relocatable section keeps its relocs.
        org     $1000
abs1:   move.l  #abs1,d0
        lea     abs2,a0
        dc.l    abs1,abs2
abs2:   rts
        section rel,code
rel1:   move.l  #rel1,d1
        move.l  #abs1,d2
        dc.l    rel1,abs2
        rts
