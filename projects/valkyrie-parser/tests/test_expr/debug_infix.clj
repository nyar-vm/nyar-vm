(block scoped
    (infix-call +
        1
        1))
(block scoped
    (infix-call ++
        2
        2))
(block scoped
    (infix-call +
        3
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call ++
        4
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call +
        4
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call ++
        5
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call ++
        6
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call +
        7
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call ++
        8
        (chain-call
            (suffix-call ))))
(block scoped
    (infix-call ++
        9
        (chain-call
            (suffix-call ))))
(block scoped
    true)
(block scoped
    (infix-call +
        0
        0.0
        (byte-literal c m)))
(block scoped
    (infix-call ++
        ""
        ""))
(block scoped
    (infix-call ++
        (template-string
            ""
            x
            "")
        (template-string
            ""
            y
            "")))