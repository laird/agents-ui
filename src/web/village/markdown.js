/* ---------- A safe markdown renderer, for an issue's body (#564) ----------
 * An issue's body and comments are text a repo's own contributors wrote, not
 * markup the village should trust: this turns headings, lists, fenced and
 * inline code, and links into HTML, and escapes everything else, so nothing
 * it did not generate itself (no raw HTML, no `javascript:` link) ever
 * reaches the page. No DOM, so node tests it
 * (tools/village-tests/markdown.test.mjs); issuedialog.js calls it. */

// Not panels.js's esc: that module pulls in layout.js, which touches `document`
// at import time, so a DOM-free node test importing this file would fail.
const mdEsc = s => String(s).replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));

/* Only a scheme a click can safely follow; anything else (a bare
 * `javascript:`, a relative path with no real target here) is left as text. */
function safeUrl(url) {
  return /^https?:\/\//i.test(url) ? url : null;
}

/* One line's inline code spans and links, text between them escaped. */
function inline(text) {
  const re = /`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/g;
  let out = '', i = 0, m;
  while ((m = re.exec(text))) {
    out += mdEsc(text.slice(i, m.index));
    if (m[1] !== undefined) {
      out += `<code>${mdEsc(m[1])}</code>`;
    } else {
      const url = safeUrl(m[3]);
      out += url ? `<a href="${mdEsc(url)}" target="_blank" rel="noopener">${mdEsc(m[2])}</a>` : mdEsc(m[0]);
    }
    i = re.lastIndex;
  }
  return out + mdEsc(text.slice(i));
}

const HEADING = /^(#{1,6})\s+(.*)$/;
const UL_ITEM = /^[-*]\s+(.*)$/;
const OL_ITEM = /^\d+\.\s+(.*)$/;
const FENCE = /^```/;

/* Markdown text to safe HTML: headings, unordered and ordered lists, fenced
 * and inline code, links, and paragraphs (a blank line breaks one; a single
 * line break inside one is kept as a break, since an issue body's own line
 * breaks are usually meant to show). Everything else is escaped text. */
export function renderMarkdown(text) {
  const lines = String(text || '').replace(/\r\n/g, '\n').split('\n');
  const out = [];
  let para = [], list = null; // list: { tag, items }
  const flushPara = () => { if (para.length) { out.push(`<p>${para.map(inline).join('<br>')}</p>`); para = []; } };
  const flushList = () => { if (list) { out.push(`<${list.tag}>${list.items.map(it => `<li>${inline(it)}</li>`).join('')}</${list.tag}>`); list = null; } };
  for (let k = 0; k < lines.length; k++) {
    const line = lines[k];
    if (FENCE.test(line)) {
      flushPara(); flushList();
      const code = [];
      k++;
      while (k < lines.length && !FENCE.test(lines[k])) { code.push(lines[k]); k++; }
      out.push(`<pre><code>${mdEsc(code.join('\n'))}</code></pre>`);
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading) { flushPara(); flushList(); const n = heading[1].length; out.push(`<h${n}>${inline(heading[2])}</h${n}>`); continue; }
    const ul = UL_ITEM.exec(line), ol = OL_ITEM.exec(line);
    if (ul || ol) {
      flushPara();
      const tag = ul ? 'ul' : 'ol', item = (ul || ol)[1];
      if (list && list.tag !== tag) flushList();
      if (!list) list = { tag, items: [] };
      list.items.push(item);
      continue;
    }
    flushList();
    if (line.trim() === '') { flushPara(); continue; }
    para.push(line);
  }
  flushPara(); flushList();
  return out.join('');
}
