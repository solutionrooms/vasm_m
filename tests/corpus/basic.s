	section code,code
start:	move.l	#$deadbeef,d0
	lea	msg(pc),a0
	moveq	#0,d1
.loop:	move.b	(a0)+,d2
	beq.s	.done
	add.l	d2,d1
	bra.s	.loop
.done:	rts
msg:	dc.b	"hello",0
	even
