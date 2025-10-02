import { describe, it, expect, vi, beforeEach } from 'vitest';
import issuesIntegration from '../index';

describe('issuesIntegration', () => {
	it('returns integration with correct name', () => {
		const integration = issuesIntegration();

		expect(integration.name).toBe('@forgepoint/astro-integration-issues');
	});

	it('has astro:config:setup hook', () => {
		const integration = issuesIntegration();

		expect(integration.hooks).toHaveProperty('astro:config:setup');
		expect(typeof integration.hooks?.['astro:config:setup']).toBe('function');
	});

	describe('slot registration', () => {
		it('does not register slot when slotRegistry not provided', () => {
			const integration = issuesIntegration();
			const injectRoute = vi.fn();
            const slotRegistry = {
                repoTabs: [],
                groupTabs: [],
                homepageWidgets: [],
                actions: [],
            };

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(slotRegistry.repoTabs).toHaveLength(0);
		});

		it('registers repo tab slot when slotRegistry provided', () => {
            const slotRegistry = {
                repoTabs: [],
                groupTabs: [],
                homepageWidgets: [],
                actions: [],
            };
			const integration = issuesIntegration({ slotRegistry });
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(slotRegistry.repoTabs).toHaveLength(1);
			expect(slotRegistry.repoTabs[0]).toMatchObject({
				id: 'issues',
				label: 'Issues',
			});
			expect(slotRegistry.repoTabs[0].componentPath).toContain('IssuesTab.vue');
		});

		it('registers slot with correct order', () => {
            const slotRegistry = {
                repoTabs: [],
                groupTabs: [],
                homepageWidgets: [],
                actions: [],
            };
			const integration = issuesIntegration({ slotRegistry });
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(slotRegistry.repoTabs[0].order).toBe(10);
		});

		it('does not register group tabs or homepage widgets', () => {
            const slotRegistry = {
                repoTabs: [],
                groupTabs: [],
                homepageWidgets: [],
                actions: [],
            };
			const integration = issuesIntegration({ slotRegistry });
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(slotRegistry.groupTabs).toHaveLength(0);
			expect(slotRegistry.homepageWidgets).toHaveLength(0);
		});
	});

	describe('route injection', () => {
		it('injects issue list route', () => {
			const integration = issuesIntegration();
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(injectRoute).toHaveBeenCalledWith(
				expect.objectContaining({
					pattern: '/issues',
				})
			);
		});

		it('injects issue detail route', () => {
			const integration = issuesIntegration();
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

			expect(injectRoute).toHaveBeenCalledWith(
				expect.objectContaining({
					pattern: '/issues/[id]',
				})
			);
		});

        it('injects routes even when slotRegistry not provided', () => {
			const integration = issuesIntegration();
			const injectRoute = vi.fn();

			integration.hooks?.['astro:config:setup']?.({
				injectRoute,
				config: {} as any,
				command: 'dev',
				isRestart: false,
				updateConfig: vi.fn(),
				addWatchFile: vi.fn(),
				addDevToolbarApp: vi.fn(),
				addMiddleware: vi.fn(),
				logger: {} as any,
				injectScript: vi.fn(),
			});

            // Six routes are injected by the integration
            expect(injectRoute).toHaveBeenCalledTimes(6);
        });
	});

	describe('integration options', () => {
		it('accepts empty options object', () => {
			const integration = issuesIntegration({});

			expect(integration).toBeDefined();
			expect(integration.name).toBe('@forgepoint/astro-integration-issues');
		});

		it('accepts undefined options', () => {
			const integration = issuesIntegration(undefined);

			expect(integration).toBeDefined();
			expect(integration.name).toBe('@forgepoint/astro-integration-issues');
		});
	});
});
