; hunk_exe.s with -databss (executable data hunk trimmed to initialised bytes) and -kick1hunks.
; bss hunks, chip memory, symbol table.
        section main,code
start:  lea     tab,a0
        move.l  (a0)+,d0
        jsr     (a0)
        move.l  #buf,a1
        move.l  #start,d1
        pea     tab
        jmp     sub2
sub2:   rts
        even
        section vars,data_c
tab:    dc.l    start,sub2,tab,buf
        dc.w    $1234
        dcb.l   3,start
        section stack,bss
buf:    ds.b    64
        ds.w    1
	section tail,data
	dc.l 0,0
	dc.b 0
