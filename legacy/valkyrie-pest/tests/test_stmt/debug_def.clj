(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        5:13,
    ),
    modifiers: [],
    parameters: [],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        30:38,
    ),
    modifiers: [
        "eager",
    ],
    parameters: [],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        48:56,
    ),
    modifiers: [],
    parameters: [
        FunctionParameter {
            kind: PositionalOnly,
            symbol: SymbolNode(
                Symbol {
                    name: "self",
                    scope: [],
                },
                57:61,
            ),
            modifiers: [],
            default: None,
        },
        FunctionParameter {
            kind: Deconstruct3,
            symbol: SymbolNode(
                Symbol {
                    name: "list",
                    scope: [],
                },
                66:70,
            ),
            modifiers: [],
            default: None,
        },
    ],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        92:100,
    ),
    modifiers: [],
    parameters: [
        FunctionParameter {
            kind: PositionalOnly,
            symbol: SymbolNode(
                Symbol {
                    name: "input",
                    scope: [],
                },
                101:106,
            ),
            modifiers: [],
            default: Some(
                ASTNode {
                    kind: Integer(
                        IntegerLiteral {
                            handler: "",
                            value: 0,
                        },
                    ),
                    span: 118:119,
                },
            ),
        },
    ],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        137:145,
    ),
    modifiers: [],
    parameters: [],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        182:190,
    ),
    modifiers: [],
    parameters: [],
    block: [],
})
(block scoped
    FunctionDefinition {
    symbol: SymbolNode(
        Symbol {
            name: "function",
            scope: [],
        },
        228:236,
    ),
    modifiers: [
        "eager",
    ],
    parameters: [
        FunctionParameter {
            kind: PositionalOnly,
            symbol: SymbolNode(
                Symbol {
                    name: "self",
                    scope: [],
                },
                241:245,
            ),
            modifiers: [
                "mut",
            ],
            default: None,
        },
        FunctionParameter {
            kind: BothAvailable,
            symbol: SymbolNode(
                Symbol {
                    name: "input",
                    scope: [],
                },
                250:255,
            ),
            modifiers: [],
            default: Some(
                ASTNode {
                    kind: Integer(
                        IntegerLiteral {
                            handler: "",
                            value: 0,
                        },
                    ),
                    span: 267:268,
                },
            ),
        },
        FunctionParameter {
            kind: Receiver,
            symbol: SymbolNode(
                Symbol {
                    name: "list",
                    scope: [],
                },
                274:278,
            ),
            modifiers: [],
            default: None,
        },
    ],
    block: [],
})