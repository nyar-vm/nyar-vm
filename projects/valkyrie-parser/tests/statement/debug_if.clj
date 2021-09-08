(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (else nothing)))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (else
            (block scoped
                2))))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (case
            b)
        (then
            (block scoped
                2))
        (else nothing)))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (case
            b)
        (then
            (block scoped
                2))
        (else
            (block scoped
                3))))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (case
            b)
        (then
            (block scoped
                2))
        (case
            c)
        (then
            (block scoped
                3))
        (else nothing)))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (case
            b)
        (then
            (block scoped
                2))
        (case
            c)
        (then
            (block scoped
                3))
        (else
            (block scoped
                4))))
(block scoped
    (switch
        (case
            a)
        (then
            (block scoped
                1))
        (case
            b)
        (then
            (block scoped
                2))
        (else
            (block scoped
                3))))