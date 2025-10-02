import type { AstroIntegration } from 'astro';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

interface SlotRegistry {
	repoTabs: Array<{
		id: string;
		label: string;
		componentPath: string;
		order?: number;
	}>;
	groupTabs: Array<{
		id: string;
		label: string;
		componentPath: string;
		order?: number;
	}>;
	homepageWidgets: Array<{
		id: string;
		componentPath: string;
		order?: number;
	}>;
	actions: Array<{
		id: string;
		label: string;
		scope: 'dashboard' | 'repository';
		order?: number;
		kind?: 'link' | 'handler';
		href?: string;
		handlerPath?: string;
	}>;
}

export interface IssuesIntegrationOptions {
	slotRegistry?: SlotRegistry;
}

export default function issuesIntegration(options?: IssuesIntegrationOptions): AstroIntegration {
	return {
		name: '@forgepoint/astro-integration-issues',
		hooks: {
			'astro:config:setup': ({ injectRoute }) => {
				injectRoute({
					pattern: '/issues',
					entrypoint: resolve(__dirname, './pages/IssueList.astro'),
				});
				injectRoute({
					pattern: '/[...repo]/issues',
					entrypoint: resolve(__dirname, './pages/IssueList.astro'),
				});
				injectRoute({
					pattern: '/issues/new',
					entrypoint: resolve(__dirname, './pages/NewIssue.astro'),
				});
				injectRoute({
					pattern: '/[...repo]/issues/new',
					entrypoint: resolve(__dirname, './pages/NewIssue.astro'),
				});
				injectRoute({
					pattern: '/[...repo]/issues/[number]',
					entrypoint: resolve(__dirname, './pages/IssueDetail.astro'),
				});
				injectRoute({
					pattern: '/issues/[id]',
					entrypoint: resolve(__dirname, './pages/IssueDetail.astro'),
				});

				if (options?.slotRegistry) {
					options.slotRegistry.repoTabs.push({
						id: 'issues',
						label: 'Issues',
						componentPath: resolve(__dirname, './components/IssuesTab.vue'),
						order: 10,
					});
					if (options?.slotRegistry?.actions) {
					options.slotRegistry.actions.push({
						id: 'issues.new',
						label: 'New Issue',
						scope: 'repository',
						order: 20,
						kind: 'link',
						href: '/{repository.fullPath}/issues/new',
					});
				}
				}
			},
		},
	};
}
