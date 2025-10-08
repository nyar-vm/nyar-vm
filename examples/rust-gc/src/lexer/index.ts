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

    scanTokens(): Token[] {
        while (!this.isAtEnd()) {
            this.start = this.current;
            this.scanToken();
        }

        this.addToken(TokenType.EOF);
        return this.tokens;
    }

    private scanToken(): void {
        const c = this.advance();

        switch (c) {
            // 单个字符的token
            case '(':
                this.addToken(TokenType.LEFT_PAREN);
                break;
            case ')':
                this.addToken(TokenType.RIGHT_PAREN);
                break;
            case '{':
                this.addToken(TokenType.LEFT_BRACE);
                break;
            case '}':
                this.addToken(TokenType.RIGHT_BRACE);
                break;
            case '[':
                this.addToken(TokenType.LEFT_BRACKET);
                break;
            case ']':
                this.addToken(TokenType.RIGHT_BRACKET);
                break;
            case ',':
                this.addToken(TokenType.COMMA);
                break;
            case ';':
                this.addToken(TokenType.SEMICOLON);
                break;
            case '.':
                this.addToken(TokenType.DOT);
                break;
            case '+':
                this.addToken(TokenType.PLUS);
                break;
            case '*':
                this.addToken(TokenType.STAR);
                break;
            case '-':
                this.addToken(TokenType.MINUS);
                break;

            // 可能有两个字符的token
            case '!':
                this.addToken(this.match('=') ? TokenType.BANG_EQUAL : TokenType.MINUS);
                break;
            case '=':
                this.addToken(this.match('=') ? TokenType.EQUAL_EQUAL : TokenType.EQUAL);
                break;
            case '<':
                this.addToken(this.match('=') ? TokenType.LESS_EQUAL : TokenType.LESS);
                break;
            case '>':
                this.addToken(this.match('=') ? TokenType.GREATER_EQUAL : TokenType.GREATER);
                break;

            // 注释
            case '/':
                if (this.match('/')) {
                    // 单行注释
                    while (this.peek() !== '\n' && !this.isAtEnd()) this.advance();
                } else {
                    this.addToken(TokenType.SLASH);
                }
                break;

            // 空白字符
            case ' ':
            case '\r':
            case '\t':
                // 忽略空白字符
                break;

            case '\n':
                this.line++;
                this.column = 1;
                break;

            // 字符串
            case '"':
                this.string();
                break;

            default:
                if (this.isDigit(c)) {
                    this.number();
                } else if (this.isAlpha(c)) {
                    this.identifier();
                } else {
                    this.diagnostics.error(this.span(), `Unexpected character: ${c}`);
                }
                break;
        }
    }

    private string(): void {
        while (this.peek() !== '"' && !this.isAtEnd()) {
            if (this.peek() === '\n') {
                this.line++;
                this.column = 1;
            }
            this.advance();
        }

        if (this.isAtEnd()) {
            this.diagnostics.error(this.span(), 'Unterminated string.');
            return;
        }

        // 闭合的引号
        this.advance();

        // 去掉引号，提取字符串值
        const value = this.source.substring(this.start + 1, this.current - 1);
        this.addToken(TokenType.IDENTIFIER, value);
    }

    private number(): void {
        while (this.isDigit(this.peek())) this.advance();

        // 查找小数部分
        if (this.peek() === '.' && this.isDigit(this.peekNext())) {
            // 消费 "."
            this.advance();

            while (this.isDigit(this.peek())) this.advance();
        }

        const value = this.source.substring(this.start, this.current);
        this.addToken(TokenType.NUMBER, value);
    }

    private identifier(): void {
        while (this.isAlphaNumeric(this.peek())) this.advance();

        const text = this.source.substring(this.start, this.current);
        const type = this.getKeywordType(text) || TokenType.IDENTIFIER;
        this.addToken(type, text);
    }

    private getKeywordType(text: string): TokenType | undefined {
        const keywords: Record<string, TokenType> = {
            fn: TokenType.FN,
            let: TokenType.LET,
            mut: TokenType.MUT,
            if: TokenType.IF,
            else: TokenType.ELSE,
            while: TokenType.WHILE,
            struct: TokenType.STRUCT,
            true: TokenType.TRUE,
            false: TokenType.FALSE,
        };

        return keywords[text];
    }

    private match(expected: string): boolean {
        if (this.isAtEnd()) return false;
        if (this.source[this.current] !== expected) return false;

        this.current++;
        this.column++;
        return true;
    }

    private peek(): string {
        if (this.isAtEnd()) return '\0';
        return this.source[this.current];
    }

    private peekNext(): string {
        if (this.current + 1 >= this.source.length) return '\0';
        return this.source[this.current + 1];
    }

    private isAlpha(c: string): boolean {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c === '_';
    }

    private isDigit(c: string): boolean {
        return c >= '0' && c <= '9';
    }

    private isAlphaNumeric(c: string): boolean {
        return this.isAlpha(c) || this.isDigit(c);
    }

    private is_at_end(): boolean {
        return this.current >= this.source.length;
    }

    private add_token(type: TokenType, _literal?: string): void {
        const text = this.source.substring(this.start, this.current);
        const span = new SourceSpan(this.sourceFile, this.start, this.current);
        this.tokens.push({
            type,
            lexeme: text,
            span,
        });
    }

    private span(): SourceSpan {
        return new SourceSpan(this.sourceFile, this.start, this.current);
    }
}
