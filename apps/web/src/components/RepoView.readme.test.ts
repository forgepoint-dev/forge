import { describe, it, expect } from 'vitest';
import { marked } from 'marked';
import Asciidoctor from 'asciidoctor';

describe('README rendering logic', () => {
	describe('Markdown rendering', () => {
		it('renders basic markdown', async () => {
			const content = '# Hello\n\nThis is a **test**.';
			
			marked.setOptions({
				breaks: true,
				gfm: true,
			});
			
			const html = await marked.parse(content);
			
			expect(html).toContain('<h1');
			expect(html).toContain('Hello');
			expect(html).toContain('<strong>test</strong>');
		});

		it('renders markdown with code blocks', async () => {
			const content = '```javascript\nconst x = 1;\n```';
			
			marked.setOptions({
				breaks: true,
				gfm: true,
			});
			
			const html = await marked.parse(content);
			
			expect(html).toContain('<code');
		});

		it('renders GFM tables', async () => {
			const content = '| Header |\n| ------ |\n| Cell   |';
			
			marked.setOptions({
				breaks: true,
				gfm: true,
			});
			
			const html = await marked.parse(content);
			
			expect(html).toContain('<table');
		});
	});

	describe('AsciiDoc rendering', () => {
		it('renders basic asciidoc', () => {
			const content = '= Title\n\nThis is a paragraph.';
			
			const asciidoctor = Asciidoctor();
			const html = asciidoctor.convert(content, { safe: 'safe' });
			
			expect(html).toContain('paragraph');
		});

		it('renders asciidoc with formatting', () => {
			const content = 'This is *bold* and _italic_.';
			
			const asciidoctor = Asciidoctor();
			const html = asciidoctor.convert(content, { safe: 'safe' });
			
			expect(html).toContain('<strong>bold</strong>');
			expect(html).toContain('<em>italic</em>');
		});
	});

	describe('README file detection', () => {
		it('detects README.md', () => {
			const entries = [
				{ name: 'README.md', type: 'FILE' as const, path: 'README.md', size: 100 },
				{ name: 'file.txt', type: 'FILE' as const, path: 'file.txt', size: 50 }
			];

			const readmeNames = [
				'README.md',
				'readme.md',
				'Readme.md',
				'README.markdown',
				'readme.markdown',
				'README.adoc',
				'readme.adoc',
				'README.asciidoc',
				'readme.asciidoc',
			];

			let found = null;
			for (const name of readmeNames) {
				const entry = entries.find((e) => e.name === name && e.type === 'FILE');
				if (entry) {
					found = entry.path;
					break;
				}
			}

			expect(found).toBe('README.md');
		});

		it('detects readme.md (lowercase)', () => {
			const entries = [
				{ name: 'readme.md', type: 'FILE' as const, path: 'readme.md', size: 100 },
			];

			const readmeNames = [
				'README.md',
				'readme.md',
				'Readme.md',
				'README.markdown',
				'readme.markdown',
				'README.adoc',
				'readme.adoc',
				'README.asciidoc',
				'readme.asciidoc',
			];

			let found = null;
			for (const name of readmeNames) {
				const entry = entries.find((e) => e.name === name && e.type === 'FILE');
				if (entry) {
					found = entry.path;
					break;
				}
			}

			expect(found).toBe('readme.md');
		});

		it('detects README.adoc', () => {
			const entries = [
				{ name: 'README.adoc', type: 'FILE' as const, path: 'README.adoc', size: 100 },
			];

			const readmeNames = [
				'README.md',
				'readme.md',
				'Readme.md',
				'README.markdown',
				'readme.markdown',
				'README.adoc',
				'readme.adoc',
				'README.asciidoc',
				'readme.asciidoc',
			];

			let found = null;
			for (const name of readmeNames) {
				const entry = entries.find((e) => e.name === name && e.type === 'FILE');
				if (entry) {
					found = entry.path;
					break;
				}
			}

			expect(found).toBe('README.adoc');
		});

		it('returns null when no README exists', () => {
			const entries = [
				{ name: 'file.txt', type: 'FILE' as const, path: 'file.txt', size: 50 },
				{ name: 'index.html', type: 'FILE' as const, path: 'index.html', size: 200 }
			];

			const readmeNames = [
				'README.md',
				'readme.md',
				'Readme.md',
				'README.markdown',
				'readme.markdown',
				'README.adoc',
				'readme.adoc',
				'README.asciidoc',
				'readme.asciidoc',
			];

			let found = null;
			for (const name of readmeNames) {
				const entry = entries.find((e) => e.name === name && e.type === 'FILE');
				if (entry) {
					found = entry.path;
					break;
				}
			}

			expect(found).toBeNull();
		});

		it('ignores directories with README names', () => {
			const entries = [
				{ name: 'README.md', type: 'DIRECTORY' as const, path: 'README.md', size: null },
				{ name: 'file.txt', type: 'FILE' as const, path: 'file.txt', size: 50 }
			];

			const readmeNames = [
				'README.md',
				'readme.md',
				'Readme.md',
				'README.markdown',
				'readme.markdown',
				'README.adoc',
				'readme.adoc',
				'README.asciidoc',
				'readme.asciidoc',
			];

			let found = null;
			for (const name of readmeNames) {
				const entry = entries.find((e) => e.name === name && e.type === 'FILE');
				if (entry) {
					found = entry.path;
					break;
				}
			}

			expect(found).toBeNull();
		});
	});

	describe('File extension detection', () => {
		it('identifies markdown files', () => {
			const filename = 'README.md';
			const ext = filename.split('.').pop()?.toLowerCase();
			
			expect(ext).toBe('md');
		});

		it('identifies asciidoc files (.adoc)', () => {
			const filename = 'README.adoc';
			const ext = filename.split('.').pop()?.toLowerCase();
			
			expect(ext).toBe('adoc');
		});

		it('identifies asciidoc files (.asciidoc)', () => {
			const filename = 'README.asciidoc';
			const ext = filename.split('.').pop()?.toLowerCase();
			
			expect(ext).toBe('asciidoc');
		});
	});
});
