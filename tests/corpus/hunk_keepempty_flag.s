; Empty sections are dropped unless -keepempty; sections with only symbols stay.
        section empty1,code
        section withsym,data
here:
        section empty2,bss
        section real,code
        move.l  #here,d0
        rts
