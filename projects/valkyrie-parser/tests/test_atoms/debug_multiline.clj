Suite[
    StringTemplate(
        "\n",
        x,
        "\n\n",
        y,
        "\n${{}\n",
    ),
]
Suite[
    StringLiteral {
        handler: "json",
        literal: "\n{\n    x: 1\n    y: 2\n}\n",
    },
]
