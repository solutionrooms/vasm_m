; Forward symbols may resolve to floating point: regrouping integer-looking
; additions must not wrap before the expression's eventual type is known.
x       equ y+2147483647+1
        dc.d x
y       equ 1.0
