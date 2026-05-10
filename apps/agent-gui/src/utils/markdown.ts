import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("rs", rust);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("yml", yaml);
hljs.registerLanguage("toml", rust);

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  highlight(source: string, language: string): string {
    if (language && hljs.getLanguage(language)) {
      try {
        const highlightedCode = hljs.highlight(source, { language }).value;
        return `<pre class="hljs"><code>${highlightedCode}</code></pre>`;
      } catch {
        return renderPlainCodeBlock(source);
      }
    }

    return renderPlainCodeBlock(source);
  }
});

function renderPlainCodeBlock(source: string): string {
  return `<pre class="hljs"><code>${md.utils.escapeHtml(source)}</code></pre>`;
}

export function renderMarkdown(text: string): string {
  return md.render(text);
}
