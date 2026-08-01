<script lang="ts">
	// Minimal, XSS-safe markdown renderer (no raw HTML allowed).
	// Supports: headings, bold, italic, inline/block code, lists, links, tables, blockquotes.

	let { source }: { source: string } = $props();

	function escapeHtml(s: string): string {
		return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
	}

	const HIGHLIGHT_LANGS =
		/^(js|ts|javascript|typescript|jsx|tsx|python|py|bash|sh|shell|zsh|json|yaml|yml|toml|html|css|scss|rust|go|sql|c|cpp|java|php|ruby|dockerfile)$/i;

	// Lightweight, XSS-safe syntax highlighting: single-pass tokenization on
	// the escaped source, so generated <span> markup is never re-processed.
	function highlight(code: string, lang: string): string {
		const escaped = escapeHtml(code);
		if (!HIGHLIGHT_LANGS.test(lang)) return escaped;
		const re =
			/("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`|\b0x[0-9a-fA-F]+\b|\b\d+\.?\d*(?:e[+-]?\d+)?\b|\b(true|false|null|undefined|None|True|False|nil)\b|\b(def|class|return|import|from|const|let|var|fn|func|function|if|elif|else|for|while|match|case|new|async|await|pub|use|struct|enum|impl|trait|print|echo|then|do|done|break|continue|in|of|try|catch|except|raise|yield|lambda|self|super|select|from|where|insert|update|delete|create|table|into|values)\b)/g;
		return escaped.replace(re, (m, str, num, bool, kw) => {
			const cls = str !== undefined
				? 'text-emerald-300'
				: num !== undefined
					? 'text-amber-300'
					: bool !== undefined
						? 'text-violet-300'
						: 'text-sky-300';
			return `<span class="${cls}">${m}</span>`;
		});
	}

	function inline(text: string): string {
		let t = escapeHtml(text);
		// code first
		t = t.replace(/`([^`]+)`/g, '<code>$1</code>');
		// bold
		t = t.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
		// italic
		t = t.replace(/\*([^*]+)\*/g, '<em>$1</em>');
		// links (http/https only)
		t = t.replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer" class="text-accent-400 underline underline-offset-2 hover:text-accent-300">$1</a>');
		return t;
	}

	function render(src: string): string {
		const lines = src.replace(/\r\n/g, '\n').split('\n');
		const out: string[] = [];
		let i = 0;
		let inCode = false;
		let codeBuf: string[] = [];
		let codeLang = '';
		let listType: '' | 'ul' | 'ol' = '';
		let tableBuf: string[] = [];
		let quote = false;

		const flushList = () => {
			if (listType) {
				out.push(`</${listType}>`);
				listType = '';
			}
		};
		const flushCode = () => {
			if (codeBuf.length || inCode) {
				const code = escapeHtml(codeBuf.join('\n'));
				const hl = highlight(code, codeLang);
				const langAttr = codeLang ? ` class="language-${escapeHtml(codeLang)}"` : '';
				out.push(`<pre class="group/block"><div class="mb-1 flex items-center justify-between px-1 text-[10px] uppercase tracking-wider text-zinc-600">${codeLang ? `<span>${escapeHtml(codeLang)}</span>` : '<span></span>'}${codeBuf.length > 0 ? '<span class="opacity-0 transition group-hover/block:opacity-100">code</span>' : ''}</div><code${langAttr}>${hl}</code></pre>`);
				codeBuf = [];
				codeLang = '';
			}
		};
		const flushTable = () => {
			if (tableBuf.length) {
				const rows = tableBuf.map((r) => r.replace(/\|/g, '</td><td>'));
				const header = `<table class="my-2 w-full border-collapse text-sm"><thead><tr><th class="border border-zinc-700 px-2 py-1 text-left">${rows[0]}</th></tr></thead><tbody>` + rows.slice(1).map((r) => `<tr><td class="border border-zinc-700 px-2 py-1">${r}</td></tr>`).join('') + '</tbody></table>';
				out.push(header);
				tableBuf = [];
			}
		};

		for (; i < lines.length; i++) {
			const line = lines[i];

			// fenced code
			if (line.trim().startsWith('```')) {
				if (!inCode) {
					flushList();
					flushTable();
					inCode = true;
					codeBuf = [];
					codeLang = line.trim().slice(3).trim();
				} else {
					inCode = false;
					flushCode();
				}
				continue;
			}
			if (inCode) {
				codeBuf.push(line);
				continue;
			}

			const trimmed = line.trim();
			if (trimmed === '') {
				flushList();
				flushTable();
				continue;
			}

			// table row
			if (trimmed.startsWith('|') && trimmed.endsWith('|')) {
				const cells = trimmed.split('|').filter((c) => c.trim() !== '');
				if (cells.every((c) => /^:?-{2,}:?$/.test(c.trim()))) continue; // separator
				flushList();
				tableBuf.push(cells.map((c) => inline(c.trim())).join('|'));
				continue;
			}

			// headings
			const h = /^(#{1,3})\s+(.*)$/.exec(trimmed);
			if (h) {
				flushList();
				flushTable();
				const level = h[1].length;
				out.push(`<h${level}>${inline(h[2])}</h${level}>`);
				continue;
			}

			// blockquote
			if (trimmed.startsWith('>')) {
				flushList();
				flushTable();
				quote = true;
				out.push(`<blockquote>${inline(trimmed.replace(/^>\s?/, ''))}</blockquote>`);
				continue;
			}

			// unordered list
			const ul = /^[-*]\s+(.*)$/.exec(trimmed);
			if (ul) {
				flushTable();
				if (listType !== 'ul') {
					flushList();
					out.push('<ul>');
					listType = 'ul';
				}
				out.push(`<li>${inline(ul[1])}</li>`);
				continue;
			}

			// ordered list
			const ol = /^\d+[.)]\s+(.*)$/.exec(trimmed);
			if (ol) {
				flushTable();
				if (listType !== 'ol') {
					flushList();
					out.push('<ol>');
					listType = 'ol';
				}
				out.push(`<li>${inline(ol[1])}</li>`);
				continue;
			}

			// horizontal rule
			if (/^(-{3,}|\*{3,})$/.test(trimmed)) {
				flushList();
				flushTable();
				out.push('<hr class="my-3 border-zinc-800">');
				continue;
			}

			// plain paragraph
			flushList();
			flushTable();
			out.push(`<p>${inline(line)}</p>`);
		}
		flushList();
		flushTable();
		if (inCode) flushCode();
		return out.join('');
	}

	const rendered = $derived(render(source));
</script>

{@html rendered}
