import type MarkdownIt from 'markdown-it';

/** 将 ```mermaid 代码块转为 VitePress 2 客户端组件。 */
export function mermaidMdPlugin(md: MarkdownIt) {
    const defaultFence = md.renderer.rules.fence!;

    md.renderer.rules.fence = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const lang = token.info.trim();

        if (lang === 'mermaid') {
            const code = md.utils.escapeHtml(token.content.trim());
            return `<VitePressMermaid code="${code}" />\n`;
        }

        return defaultFence(tokens, idx, options, env, self);
    };
}
