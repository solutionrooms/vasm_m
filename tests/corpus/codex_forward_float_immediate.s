; This also affects an ordinary 68000 instruction, not just data directives.
x       equ y+2147483647+1
        move.l #x,d0
y       equ 1.0
