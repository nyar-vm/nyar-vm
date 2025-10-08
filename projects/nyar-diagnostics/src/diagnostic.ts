import { SourceSpan } from './source-span.js';
import { DiagnosticSeverity } from './diagnostic-severity';
import { DiagnosticMessage } from './diagnostic-message';

/**
 * Represents a diagnostic (error, warning, info, hint)
 */
export class Diagnostic {
    constructor(
        public readonly severity: DiagnosticSeverity,
        public readonly code: string,
        public readonly title: string,
        public readonly messages: DiagnosticMessage[],
        public readonly span: SourceSpan | null = null,
        public readonly related: Diagnostic[] = []
    ) {}

    /**
     * Create an error diagnostic
     */
    static error(
        code: string,
        title: string,
        message: string,
        span?: SourceSpan,
        suggestion?: string
    ): Diagnostic {
        const message_obj: DiagnosticMessage = { message };
        if (suggestion) {
            message_obj.suggestion = suggestion;
        }
        return new Diagnostic(DiagnosticSeverity.Error, code, title, [message_obj], span);
    }

    /**
     * Create a warning diagnostic
     */
    static warning(
        code: string,
        title: string,
        message: string,
        span?: SourceSpan,
        suggestion?: string
    ): Diagnostic {
        const message_obj: DiagnosticMessage = { message };
        if (suggestion) {
            message_obj.suggestion = suggestion;
        }
        return new Diagnostic(DiagnosticSeverity.Warning, code, title, [message_obj], span);
    }

    /**
     * Create an info diagnostic
     */
    static info(code: string, title: string, message: string, span?: SourceSpan): Diagnostic {
        return new Diagnostic(DiagnosticSeverity.Info, code, title, [{ message }], span);
    }

    /**
     * Create a hint diagnostic
     */
    static hint(code: string, title: string, message: string, span?: SourceSpan): Diagnostic {
        return new Diagnostic(DiagnosticSeverity.Hint, code, title, [{ message }], span);
    }

    /**
     * Add a related diagnostic
     */
    with_related(diagnostic: Diagnostic): Diagnostic {
        return new Diagnostic(this.severity, this.code, this.title, this.messages, this.span, [
            ...this.related,
            diagnostic,
        ]);
    }

    /**
     * Add multiple related diagnostics
     */
    with_related_many(diagnostics: Diagnostic[]): Diagnostic {
        return new Diagnostic(this.severity, this.code, this.title, this.messages, this.span, [
            ...this.related,
            ...diagnostics,
        ]);
    }

    /**
     * Add additional messages
     */
    with_message(message: string, suggestion?: string, help?: string): Diagnostic {
        const new_message: DiagnosticMessage = { message };
        if (suggestion !== undefined) {
            new_message.suggestion = suggestion;
        }
        if (help !== undefined) {
            new_message.help = help;
        }
        const new_messages = [...this.messages, new_message];
        return new Diagnostic(
            this.severity,
            this.code,
            this.title,
            new_messages,
            this.span,
            this.related
        );
    }

    /**
     * Get the primary message
     */
    get primaryMessage(): string {
        return this.messages[0]?.message ?? '';
    }

    /**
     * Get the primary suggestion
     */
    get primarySuggestion(): string | undefined {
        return this.messages[0]?.suggestion;
    }

    /**
     * Get the help text
     */
    get help(): string | undefined {
        return this.messages[0]?.help;
    }
}
