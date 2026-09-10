; local labels, macro-local labels, reused local names
	moveq	#3,d0
.loop:	dbf	d0,.loop
sub2:	moveq	#4,d0
.loop:	nop
	dbf	d0,.loop
	rts
lp$	nop
	bra.s	lp$
sub3:	nop
lp$	bra	lp$

WAIT	macro
\@wait:	tst.w	\1
	bne.s	\@wait
	endm

	WAIT	d0
	WAIT	d1
	WAIT	(a0)

ADDX2	macro
	add.\0	\1,\2
	add.\0	\1,\2
	endm

	ADDX2.w	d0,d1
	ADDX2.l	#4,a0
	ADDX2.b	#1,(a0)

	rept	3
	nop
	endr
CNT	set	0
	rept	4
CNT	set	CNT+1
	dc.b	CNT
	endr
	even
