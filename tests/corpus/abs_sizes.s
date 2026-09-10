; absolute word sign extension and abs.w/abs.l selection
	move.w	$1234,d0
	move.w	$7fff,d0
	move.w	$8000,d0
	move.w	$ffff8000,d0
	move.w	$ffffffff,d0
	move.w	$00ff0000,d0
	move.w	$1234.w,d0
	move.w	$1234.l,d0
	jmp	$c00000
	jsr	$ff0000
	clr.w	$a10001
	move.b	#1,$c00011
	move.w	#$8000,$c00004.l
