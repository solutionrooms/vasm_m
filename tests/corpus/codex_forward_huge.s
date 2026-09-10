; 68000-only instructions do not exclude wide integer data expressions.
x       equ y+2147483647+1
        dc.q x
y       equ $100000000
