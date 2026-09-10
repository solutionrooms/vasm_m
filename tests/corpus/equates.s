; forward equates, mutable set, expressions
	move.l	#FWD,d0
	move.w	#VAL,d1
	moveq	#SMALL,d2
	move.l	#VAL2,d3
VAL	equ	$1234
FWD	equ	VAL*2+1
SMALL	equ	-5
VAL2	=	VAL<<8
CNT	set	1
	dc.w	CNT
CNT	set	CNT+1
	dc.w	CNT
CNT	set	CNT*10
	dc.w	CNT
	dc.l	VAL/3,VAL//7,-VAL,~VAL,VAL&$ff,VAL|1,VAL^$ffff
	dc.l	VAL>>4,VAL<<4,-16>>2,(VAL=VAL),(VAL<>VAL),(VAL<$2000)
	dc.b	'A',"BC",0
	dc.w	'AB'
	dc.l	'ABCD'
	dc.l	%1010,@17,$ff,255
	dc.l	*
	dc.l	*-4
