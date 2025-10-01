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
				}
			},
		},
	};
}
