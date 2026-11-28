// A safe markdown renderer for an issue's body (src/web/village/markdown.js, #564):
//   node --test tools/village-tests/
// Also run by `cargo test village_node_tests`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderMarkdown } from '../../src/web/village/markdown.js';

test('headings, paragraphs and line breaks', () => {
  assert.equal(renderMarkdown('# Title\n\nFirst line\nsecond line.'), '<h1>Title</h1><p>First line<br>second line.</p>');
  assert.equal(renderMarkdown('### Sub'), '<h3>Sub</h3>');
});

test('unordered and ordered lists', () => {
  assert.equal(renderMarkdown('- one\n- two'), '<ul><li>one</li><li>two</li></ul>');
  assert.equal(renderMarkdown('1. first\n2. second'), '<ol><li>first</li><li>second</li></ol>');
  // A new kind of marker starts a new list rather than joining the old one.
  assert.equal(renderMarkdown('- one\n1. two'), '<ul><li>one</li></ul><ol><li>two</li></ol>');
});

test('inline and fenced code', () => {
  assert.equal(renderMarkdown('Run `cargo test`.'), '<p>Run <code>cargo test</code>.</p>');
  assert.equal(renderMarkdown('```\nlet x = 1;\n```'), '<pre><code>let x = 1;</code></pre>');
  // Code is not re-parsed for markdown or escaped twice.
  assert.equal(renderMarkdown('```\n<script>\n```'), '<pre><code>&lt;script&gt;</code></pre>');
});

test('links keep only a safe scheme', () => {
  assert.equal(renderMarkdown('[issue](https://github.com/laird/agents-ui/issues/7)'), '<p><a href="https://github.com/laird/agents-ui/issues/7" target="_blank" rel="noopener">issue</a></p>');
  // javascript: and other unsafe schemes are left as literal, escaped text.
  assert.equal(renderMarkdown('[click](javascript:alert(1))'), '<p>[click](javascript:alert(1))</p>');
});

test('nothing it did not generate reaches the page', () => {
  assert.equal(renderMarkdown('<img src=x onerror=alert(1)>'), '<p>&lt;img src=x onerror=alert(1)&gt;</p>');
  assert.equal(renderMarkdown('Tom & Jerry < 3'), '<p>Tom &amp; Jerry &lt; 3</p>');
});

test('blank input renders nothing', () => {
  assert.equal(renderMarkdown(''), '');
  assert.equal(renderMarkdown(null), '');
});
