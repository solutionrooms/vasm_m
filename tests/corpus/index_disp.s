; 8-bit indexed displacement limits and PC-relative bases
base:	move.l	127(a0,d0.w),d1
	move.l	-128(a0,d0.l),d1
	move.l	0(a0,d0.w),d1
	move.l	(a0,d0.w),d1
	move.b	(tbl-base)-2(pc,d1.w),d2
	lea	tbl(pc),a0
	lea	tbl(pc,d0.w),a0
	move.w	tbl(pc),d0
	move.w	0(a0),d0
	move.w	(0,a0),d0
	move.w	32767(a0),d0
	move.w	-32768(a0),d0
tbl:	dc.b	1,2,3,4
	even
