import { describe, it, expect } from 'vitest';
import { createSlotRegistry, createSlotPlugin } from './slot-plugin';

describe('createSlotRegistry', () => {
	it('creates empty registry with all slot types', () => {
		const registry = createSlotRegistry();

		expect(registry).toHaveProperty('repoTabs');
		expect(registry).toHaveProperty('groupTabs');
		expect(registry).toHaveProperty('homepageWidgets');
		expect(registry.repoTabs).toEqual([]);
		expect(registry.groupTabs).toEqual([]);
		expect(registry.homepageWidgets).toEqual([]);
	});
});

describe('createSlotPlugin', () => {
	it('creates plugin with correct name', () => {
		const registry = createSlotRegistry();
		const plugin = createSlotPlugin(registry);

		expect(plugin.name).toBe('forge-slot-plugin');
	});

	it('exposes registry via __registry', () => {
		const registry = createSlotRegistry();
		const plugin = createSlotPlugin(registry);

		expect(plugin.__registry).toBe(registry);
	});

	describe('resolveId', () => {
		it('resolves repo-tabs virtual module', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.resolveId?.('virtual:forge/slots/repo-tabs', '', {});
			expect(result).toBe('\0virtual:forge/slots/repo-tabs');
		});

		it('resolves group-tabs virtual module', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.resolveId?.('virtual:forge/slots/group-tabs', '', {});
			expect(result).toBe('\0virtual:forge/slots/group-tabs');
		});

		it('resolves homepage-widgets virtual module', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.resolveId?.('virtual:forge/slots/homepage-widgets', '', {});
			expect(result).toBe('\0virtual:forge/slots/homepage-widgets');
		});

		it('returns undefined for non-virtual modules', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.resolveId?.('./regular-module.ts', '', {});
			expect(result).toBeUndefined();
		});
	});

	describe('load', () => {
		it('generates empty array for repo-tabs with no registrations', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toBe('\n\nexport const repoTabs = [\n  \n];');
		});

		it('generates module with single repo-tab registration', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push({
				id: 'test-tab',
				label: 'Test Tab',
				componentPath: './components/TestTab.vue',
				order: 10,
			});
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toContain("import Component0 from './components/TestTab.vue';");
			expect(result).toContain("id: 'test-tab'");
			expect(result).toContain("label: 'Test Tab'");
			expect(result).toContain('order: 10');
		});

		it('generates module with multiple repo-tab registrations', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push(
				{
					id: 'tab-1',
					label: 'Tab 1',
					componentPath: './Tab1.vue',
					order: 10,
				},
				{
					id: 'tab-2',
					label: 'Tab 2',
					componentPath: './Tab2.vue',
					order: 20,
				},
			);
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toContain("import Component0 from './Tab1.vue';");
			expect(result).toContain("import Component1 from './Tab2.vue';");
			expect(result).toContain("id: 'tab-1'");
			expect(result).toContain("id: 'tab-2'");
		});

		it('sorts repo-tabs by order property', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push(
				{
					id: 'tab-high',
					label: 'High Order',
					componentPath: './High.vue',
					order: 100,
				},
				{
					id: 'tab-low',
					label: 'Low Order',
					componentPath: './Low.vue',
					order: 10,
				},
				{
					id: 'tab-medium',
					label: 'Medium Order',
					componentPath: './Medium.vue',
					order: 50,
				},
			);
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {}) as string;

			const lowIndex = result.indexOf("id: 'tab-low'");
			const mediumIndex = result.indexOf("id: 'tab-medium'");
			const highIndex = result.indexOf("id: 'tab-high'");

			expect(lowIndex).toBeLessThan(mediumIndex);
			expect(mediumIndex).toBeLessThan(highIndex);
		});

		it('treats undefined order as 0', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push(
				{
					id: 'tab-no-order',
					label: 'No Order',
					componentPath: './NoOrder.vue',
				},
				{
					id: 'tab-with-order',
					label: 'With Order',
					componentPath: './WithOrder.vue',
					order: 10,
				},
			);
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {}) as string;

			const noOrderIndex = result.indexOf("id: 'tab-no-order'");
			const withOrderIndex = result.indexOf("id: 'tab-with-order'");

			expect(noOrderIndex).toBeLessThan(withOrderIndex);
		});

		it('generates module for group-tabs', () => {
			const registry = createSlotRegistry();
			registry.groupTabs.push({
				id: 'group-tab',
				label: 'Group Tab',
				componentPath: './GroupTab.vue',
				order: 5,
			});
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/group-tabs', {});
			expect(result).toContain("import Component0 from './GroupTab.vue';");
			expect(result).toContain("id: 'group-tab'");
			expect(result).toContain("label: 'Group Tab'");
			expect(result).toContain('export const groupTabs =');
		});

		it('generates module for homepage-widgets', () => {
			const registry = createSlotRegistry();
			registry.homepageWidgets.push({
				id: 'widget',
				label: 'Widget',
				componentPath: './Widget.vue',
				order: 1,
			});
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/homepage-widgets', {});
			expect(result).toContain("import Component0 from './Widget.vue';");
			expect(result).toContain("id: 'widget'");
			expect(result).not.toContain('label:');
			expect(result).toContain('export const homepageWidgets =');
		});

		it('returns undefined for non-virtual modules', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('./regular-module.ts', {});
			expect(result).toBeUndefined();
		});
	});

	describe('slot registry mutations', () => {
		it('reflects registry changes in generated module', () => {
			const registry = createSlotRegistry();
			const plugin = createSlotPlugin(registry);

			let result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toBe('\n\nexport const repoTabs = [\n  \n];');

			registry.repoTabs.push({
				id: 'new-tab',
				label: 'New Tab',
				componentPath: './NewTab.vue',
			});

			result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toContain("id: 'new-tab'");
		});
	});

	describe('edge cases', () => {
		it('handles slots with special characters in labels', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push({
				id: 'special',
				label: "Tab's Label with \"quotes\"",
				componentPath: './Special.vue',
			});
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toContain('label: ');
		});

		it('handles slots with paths containing special characters', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push({
				id: 'special-path',
				label: 'Special Path',
				componentPath: '@scope/package/components/Tab.vue',
			});
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {});
			expect(result).toContain('@scope/package/components/Tab.vue');
		});

		it('handles negative order values', () => {
			const registry = createSlotRegistry();
			registry.repoTabs.push(
				{
					id: 'negative',
					label: 'Negative',
					componentPath: './Negative.vue',
					order: -10,
				},
				{
					id: 'positive',
					label: 'Positive',
					componentPath: './Positive.vue',
					order: 10,
				},
			);
			const plugin = createSlotPlugin(registry);

			const result = plugin.load?.('\0virtual:forge/slots/repo-tabs', {}) as string;

			const negativeIndex = result.indexOf("id: 'negative'");
			const positiveIndex = result.indexOf("id: 'positive'");

			expect(negativeIndex).toBeLessThan(positiveIndex);
		});
	});
});
