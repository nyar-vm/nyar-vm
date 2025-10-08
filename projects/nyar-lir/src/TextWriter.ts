export class TextWriter {
    private content: string[];

    constructor() {
        this.content = [];
    }

    write_line(line: string = ''): this {
        this.content.push(line);
        return this;
    }

    write(text: string): this {
        if (this.content.length === 0) {
            this.content.push(text);
        } else {
            this.content[this.content.length - 1] += text;
        }
        return this;
    }

    write_formatted(format: string, ...args: unknown[]): this {
        let result = format;
        for (let i = 0; i < args.length; i++) {
            result = result.replace(`{${i}}`, String(args[i]));
        }
        return this.write(result);
    }

    indent(level: number = 1): this {
        const spaces = '    '.repeat(level);
        if (this.content.length > 0) {
            this.content[this.content.length - 1] = spaces + this.content[this.content.length - 1];
        }
        return this;
    }

    get_content(): string {
        return this.content.join('\n');
    }

    clear(): this {
        this.content = [];
        return this;
    }

    to_string(): string {
        return this.get_content();
    }
}
