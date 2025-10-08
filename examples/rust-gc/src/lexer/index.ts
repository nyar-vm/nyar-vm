import { DiagnosticCollector, SourceFile, SourceSpan } from '@nyar/diagnostics';

export enum TokenType {
    // 字面量
    NUMBER = 'NUMBER',
    IDENTIFIER = 'IDENTIFIER',

    // 关键字
    FN = 'FN',
    LET = 'LET',
    MUT = 'MUT',
    IF = 'IF',
    ELSE = 'ELSE',
    WHILE = 'WHILE',
    STRUCT = 'STRUCT',
    TRUE = 'TRUE',
    FALSE = 'FALSE',

    // 运算符
    PLUS = 'PLUS',
    MINUS = 'MINUS',
    STAR = 'STAR',
    SLASH = 'SLASH',
    EQUAL = 'EQUAL',
    EQUAL_EQUAL = 'EQUAL_EQUAL',
    BANG_EQUAL = 'BANG_EQUAL',
    LESS = 'LESS',
    GREATER = 'GREATER',
    LESS_EQUAL = 'LESS_EQUAL',
    GREATER_EQUAL = 'GREATER_EQUAL',

    // 分隔符
    LEFT_PAREN = 'LEFT_PAREN',
    RIGHT_PAREN = 'RIGHT_PAREN',
    LEFT_BRACE = 'LEFT_BRACE',
    RIGHT_BRACE = 'RIGHT_BRACE',
    LEFT_BRACKET = 'LEFT_BRACKET',
    RIGHT_BRACKET = 'RIGHT_BRACKET',
    COMMA = 'COMMA',
    SEMICOLON = 'SEMICOLON',
    DOT = 'DOT',
    ARROW = 'ARROW',

    // 特殊
    EOF = 'EOF',
    NEWLINE = 'NEWLINE',
}

export interface Token {
    type: TokenType;
    lexeme: string;
    span: SourceSpan;
}

export class Lexer {
    private source: string;
    private tokens: Token[] = [];
    private start = 0;
    private current = 0;
    private line = 1;
    private column = 1;

    constructor(
        private sourceFile: SourceFile,
        private diagnostics: DiagnosticCollector
    ) {
        this.source = sourceFile.content;
    }

    scan_tokens(): Token[] {
        while (!this.is_at_end()) {
            this.start = this.current;
            this.scan_token();
        }

        this.tokens.push({ type: TokenType.EOF, lexeme: '', line: this.line });
        return this.tokens;
    }

    private is_at_end(): boolean {
        return this.current >= this.source.length;
    }

    private scan_token(): void {
        const c = this.advance();
        switch (c) {
            case '(':
                this.add_token(TokenType.LEFT_PAREN);
                break;
            case ')':
                this.add_token(TokenType.RIGHT_PAREN);
                break;
            case '{':
                this.add_token(TokenType.LEFT_BRACE);
                break;
            case '}':
                this.add_token(TokenType.RIGHT_BRACE);
                break;
            case ',':
                this.add_token(TokenType.COMMA);
                break;
            case '.':
                this.add_token(TokenType.DOT);
                break;
            case '-':
                this.add_token(TokenType.MINUS);
                break;
            case '+':
                this.add_token(TokenType.PLUS);
                break;
            case ';':
                this.add_token(TokenType.SEMICOLON);
                break;
            case '*':
                this.add_token(TokenType.STAR);
                break;
            case '!':
                this.add_token(this.match('=') ? TokenType.BANG_EQUAL : TokenType.BANG);
                break;
            case '=':
                this.add_token(this.match('=') ? TokenType.EQUAL_EQUAL : TokenType.EQUAL);
                break;
            case '<':
                this.add_token(this.match('=') ? TokenType.LESS_EQUAL : TokenType.LESS);
                break;
            case '>':
                this.add_token(this.match('=') ? TokenType.GREATER_EQUAL : TokenType.GREATER);
                break;
            case '/':
                if (this.match('/')) {
                    while (this.peek() !== '\n' && !this.is_at_end()) this.advance();
                } else {
                    this.add_token(TokenType.SLASH);
                }
                break;
            case ' ':
            case '\r':
            case '\t':
                break;
            case '\n':
                this.line++;
                break;
            case '"':
                this.string();
                break;
            default:
                if (this.is_digit(c)) {
                    this.number();
                } else if (this.is_alpha(c)) {
                    this.identifier();
                } else {
                    this.diagnostics.error(this.line, `Unexpected character: ${c}`);
                }
                break;
        }
    }

    private string(): void {
        while (this.peek() !== '"' && !this.is_at_end()) {
            if (this.peek() === '\n') this.line++;
            this.advance();
        }

        if (this.is_at_end()) {
            this.diagnostics.error(this.line, 'Unterminated string.');
            return;
        }

        this.advance();

        const value = this.source.substring(this.start + 1, this.current - 1);
        this.add_token(TokenType.STRING, value);
    }

    private number(): void {
        while (this.is_digit(this.peek())) this.advance();

        if (this.peek() === '.' && this.is_digit(this.peek_next())) {
            this.advance();
            while (this.is_digit(this.peek())) this.advance();
        }

        const value = this.source.substring(this.start, this.current);
        this.add_token(TokenType.NUMBER, value);
    }

    private identifier(): void {
        while (this.is_alpha_numeric(this.peek())) this.advance();

        const text = this.source.substring(this.start, this.current);
        let type = this.get_keyword_type(text);
        if (type === undefined) type = TokenType.IDENTIFIER;
        this.add_token(type);
    }

    private get_keyword_type(name: string): TokenType | undefined {
        // eslint-disable-next-line @typescript-eslint/naming-convention
        const keywords: Record<string, TokenType> = {
            and: TokenType.AND,
            class: TokenType.CLASS,
            else: TokenType.ELSE,
            false: TokenType.FALSE,
            for: TokenType.FOR,
            fun: TokenType.FUN,
            if: TokenType.IF,
            nil: TokenType.NIL,
            or: TokenType.OR,
            print: TokenType.PRINT,
            return: TokenType.RETURN,
            super: TokenType.SUPER,
            this: TokenType.THIS,
            true: TokenType.TRUE,
            var: TokenType.VAR,
            while: TokenType.WHILE,
            struct: TokenType.STRUCT,
            impl: TokenType.IMPL,
            trait: TokenType.TRAIT,
            let: TokenType.LET,
            mut: TokenType.MUT,
            ref: TokenType.REF,
            match: TokenType.MATCH,
            enum: TokenType.ENUM,
            mod: TokenType.MOD,
            use: TokenType.USE,
            pub: TokenType.PUB,
            priv: TokenType.PRIV,
            static: TokenType.STATIC,
            async: TokenType.ASYNC,
            await: TokenType.AWAIT,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Box: TokenType.BOX,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Vec: TokenType.VEC,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Option: TokenType.OPTION,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Result: TokenType.RESULT,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Some: TokenType.SOME,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            None: TokenType.NONE,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Ok: TokenType.OK,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            Err: TokenType.ERR,
            // eslint-disable-next-line @typescript-eslint/naming-convention
            String: TokenType.STRING_TYPE,
            i32: TokenType.I32,
            i64: TokenType.I64,
            f32: TokenType.F32,
            f64: TokenType.F64,
            bool: TokenType.BOOL,
            char: TokenType.CHAR,
            usize: TokenType.USIZE,
            isize: TokenType.ISIZE,
            u32: TokenType.U32,
            u64: TokenType.U64,
        };
        return keywords[name];
    }

    private peek_next(): string {
        if (this.current + 1 >= this.source.length) return '\0';
        return this.source.charAt(this.current + 1);
    }

    private is_alpha(c: string): boolean {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c === '_';
    }

    private is_digit(c: string): boolean {
        return c >= '0' && c <= '9';
    }

    private is_alpha_numeric(c: string): boolean {
        return this.is_alpha(c) || this.is_digit(c);
    }

    private advance(): string {
        return this.source.charAt(this.current++);
    }

    private add_token(type: TokenType, literal?: unknown): void {
        const text = this.source.substring(this.start, this.current);
        this.tokens.push({ type, lexeme: text, literal, line: this.line });
    }
}
