(block scoped
    (chain-call
        (slice-call
            (index
                1))))
(block scoped
    (chain-call
        (slice-call
            (index
                (table-literal
                    1)))))
(block scoped
    (chain-call
        (slice-call
            (index
                2)
            (index
                (infix-call +
                    1
                    1)))))
(block scoped
    (chain-call
        (slice-call
            (slice
                1
                1
                1))))
(block scoped
    (chain-call
        (slice-call
            (slice
                2
                2
                1)
            (slice
                (infix-call +
                    1
                    1)
                (infix-call +
                    1
                    1)
                1))))
(block scoped
    (chain-call
        (slice-call
            (slice
                1
                2
                3)
            (index
                (table-literal
                    1
                    2
                    3)))))
(block scoped
    (chain-call
        (slice-call
            (slice
                1
                -1
                (chain-call
                    (suffix-call ))))))
(block scoped
    (chain-call
        (slice-call
            (slice
                1
                (chain-call
                    (suffix-call ))
                1))))
(block scoped
    (chain-call
        (slice-call
            (slice
                2
                -1
                1))))