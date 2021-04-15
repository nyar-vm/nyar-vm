(block scoped
    a::b::c)
(block scoped
    (chain-call
        <<unimplemented ApplyArgument>>))
(block scoped
    (chain-call
        (static-call c)))
(block scoped
    (chain-call
        (static-call c)
        <<unimplemented ApplyArgument>>))
(block scoped
    (chain-call
        (static-call b::c)))
(block scoped
    (chain-call
        (static-call b::c)
        <<unimplemented ApplyArgument>>))
(block scoped
    (chain-call
        (static-call b)
        (static-call c)))
(block scoped
    (chain-call
        (static-call b)
        (static-call c)
        <<unimplemented ApplyArgument>>))
(block scoped
    (chain-call
        (static-call b)
        <<unimplemented ApplyArgument>>
        (static-call c)
        <<unimplemented ApplyArgument>>))
(block scoped
    (chain-call
        <<unimplemented ApplyArgument>>
        (static-call b)
        <<unimplemented ApplyArgument>>
        (static-call c)
        <<unimplemented ApplyArgument>>))