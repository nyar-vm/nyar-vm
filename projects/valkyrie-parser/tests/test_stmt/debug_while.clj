(block scoped
    LoopStatement {
    body: [
        ASTNode {
            kind: Suite(
                [
                    ASTNode {
                        kind: Symbol(
                            Symbol {
                                name: "looping",
                                scope: [],
                            },
                        ),
                        span: 13:20,
                    },
                ],
            ),
            span: 13:21,
        },
    ],
})
(block scoped
    LoopStatement {
    body: [
        ASTNode {
            kind: Suite(
                [
                    ASTNode {
                        kind: Symbol(
                            Symbol {
                                name: "looping",
                                scope: [],
                            },
                        ),
                        span: 40:47,
                    },
                ],
            ),
            span: 40:48,
        },
    ],
})
(block scoped
    (switch
        (case
            a)
        (then
            LoopStatement {
    body: [
        ASTNode {
            kind: IfStatement(
                IfStatement {
                    pairs: [
                        (
                            ASTNode {
                                kind: Symbol(
                                    Symbol {
                                        name: "a",
                                        scope: [],
                                    },
                                ),
                                span: 56:57,
                            },
                            [
                                ASTNode {
                                    kind: Suite(
                                        [
                                            ASTNode {
                                                kind: IfStatement(
                                                    IfStatement {
                                                        pairs: [
                                                            (
                                                                ASTNode {
                                                                    kind: Symbol(
                                                                        Symbol {
                                                                            name: "a",
                                                                            scope: [],
                                                                        },
                                                                    ),
                                                                    span: 72:73,
                                                                },
                                                                [
                                                                    ASTNode {
                                                                        kind: Suite(
                                                                            [
                                                                                ASTNode {
                                                                                    kind: Symbol(
                                                                                        Symbol {
                                                                                            name: "break",
                                                                                            scope: [],
                                                                                        },
                                                                                    ),
                                                                                    span: 89:94,
                                                                                },
                                                                            ],
                                                                        ),
                                                                        span: 89:99,
                                                                    },
                                                                ],
                                                            ),
                                                        ],
                                                        default: Some(
                                                            [
                                                                ASTNode {
                                                                    kind: Suite(
                                                                        [
                                                                            ASTNode {
                                                                                kind: Symbol(
                                                                                    Symbol {
                                                                                        name: "continue",
                                                                                        scope: [],
                                                                                    },
                                                                                ),
                                                                                span: 120:128,
                                                                            },
                                                                        ],
                                                                    ),
                                                                    span: 120:133,
                                                                },
                                                            ],
                                                        ),
                                                    },
                                                ),
                                                span: 68:134,
                                            },
                                        ],
                                    ),
                                    span: 68:135,
                                },
                            ],
                        ),
                    ],
                    default: Some(
                        [
                            ASTNode {
                                kind: Symbol(
                                    Symbol {
                                        name: "break",
                                        scope: [],
                                    },
                                ),
                                span: 56:57,
                            },
                        ],
                    ),
                },
            ),
            span: 50:157,
        },
    ],
})
        (else
            (block scoped
                nothing))))