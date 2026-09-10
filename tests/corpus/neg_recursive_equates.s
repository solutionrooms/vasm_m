; Negative reproducer kept outside corpus until the harness has a timeout.
; vasm 1.7h exits 1 with "symbol recursively defined".
; Reviewed vasm_m commit 27c4263 failed to terminate within 3 seconds.
xx      equ yy
yy      equ xx
        move.l #xx,d0
