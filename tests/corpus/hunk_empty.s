; No sections at all, only an exported equate: hunk gets a dummy .text section.
value   equ     42
        xdef    value
other   equ     value*2
