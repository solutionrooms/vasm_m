; Same as codex_merged_data_pc.s but with -align (natural alignment disables the merge).
        org $100
        dc.b "ab",1,2,"cd",3
point:  dc.w 4,*,5,*
        dc.l point,*,6,7
        rorg $200
        dc.b 8,9
        dc.l *,10,11
        dc.l *,12,13
        even
        nop
