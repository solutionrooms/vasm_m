; branch displacement boundaries: 0, +127/-128 (byte), +32767 (word)
	bra	fwd0
fwd0:	nop
	bra.s	fwd0
	bra	back
back:	bra	back
	beq	l127
	ds.b	124
l127:	nop
	bne	l128
	ds.b	126
l128:	nop
	bsr	far
	ds.b	32000
far:	rts
	bra	back2
	ds.b	32760
back2:	rts
