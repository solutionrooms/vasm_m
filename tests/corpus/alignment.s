; even, cnop, ds, dcb, align with odd sizes
	dc.b	1
	even
	dc.b	1,2,3
	cnop	0,4
	dc.b	1
	cnop	2,4
	dc.w	$1234
	ds.b	3
	even
	ds.w	2
	ds.l	1
	dcb.b	5,$aa
	dcb.w	3,$bbcc
	dcb.l	2,$deadbeef
	dc.b	"odd"
	dc.w	1
	dc.b	"x"
	dc.l	1
	align	2
	dc.b	1
	align	4
	dc.b	2
	even
