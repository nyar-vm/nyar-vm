(block scoped
    (infix-call +
        1
        (infix-call *
            2
            3)))
(block scoped
    (infix-call *
        (infix-call +
            1
            2)
        3))
(block scoped
    (infix-call +
        1
        (infix-call *
            2
            3)
        (infix-call *
            4
            5
            6)))