(block scoped
    (template-string
        "\n"
        x
        "\n\n"
        y
        "\n${{}\n"))
(block scoped
    json"\n{\n    x: 1\n    y: 2\n}\n")