; expressions between labels, PC symbol, label differences in data
start:	nop
	nop
mid:	dc.w	mid-start
	dc.w	end-mid
	dc.l	end-start
	dc.w	(end-start)/2
	move.w	#end-start,d0
	move.w	#(mid-start)*4,d0
	lea	end-start(a0),a1
	move.l	#start,d0
	move.w	#start,d0
end:
