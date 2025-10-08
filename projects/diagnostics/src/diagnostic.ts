import { SourceSpan } from './source-span.js';

/**
 * Severity levels for diagnostics
 */
export enum DiagnosticSeverity {
  Error = 'error',
  Warning = 'warning',
  Info = 'info',
  Hint = 'hint',
}

/**
 * Represents a diagnostic message with optional suggestions
 */
export interface DiagnosticMessage {
  message: string;
  suggestion?: string;
  help?: string;
}

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
    const messages: DiagnosticMessage[] = [{ message }];
    if (suggestion) {
      messages[0].suggestion = suggestion;
    }
    return new Diagnostic(DiagnosticSeverity.Error, code, title, messages, span);
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
    const messages: DiagnosticMessage[] = [{ message }];
    if (suggestion) {
      messages[0].suggestion = suggestion;
    }
    return new Diagnostic(DiagnosticSeverity.Warning, code, title, messages, span);
  }

  /**
   * Create an info diagnostic
   */
  static info(
    code: string,
    title: string,
    message: string,
    span?: SourceSpan
  ): Diagnostic {
    return new Diagnostic(DiagnosticSeverity.Info, code, title, [{ message }], span);
  }

  /**
   * Create a hint diagnostic
   */
  static hint(
    code: string,
    title: string,
    message: string,
    span?: SourceSpan
  ): Diagnostic {
    return new Diagnostic(DiagnosticSeverity.Hint, code, title, [{ message }], span);
  }

  /**
   * Add a related diagnostic
   */
  withRelated(diagnostic: Diagnostic): Diagnostic {
    return new Diagnostic(
      this.severity,
      this.code,
      this.title,
      this.messages,
      this.span,
      [...this.related, diagnostic]
    );
  }

  /**
   * Add multiple related diagnostics
   */
  withRelatedMany(diagnostics: Diagnostic[]): Diagnostic {
    return new Diagnostic(
      this.severity,
      this.code,
      this.title,
      this.messages,
      this.span,
      [...this.related, ...diagnostics]
    );
  }

  /**
   * Add additional messages
   */
  withMessage(message: string, suggestion?: string, help?: string): Diagnostic {
    const newMessages = [...this.messages, { message, suggestion, help }];
    return new Diagnostic(
      this.severity,
      this.code,
      this.title,
      newMessages,
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