import type { AstroIntegration } from 'astro';

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
					entrypoint: '@forgepoint/astro-integration-issues/pages/IssueList.astro',
				});
				injectRoute({
					pattern: '/issues/[id]',
					entrypoint: '@forgepoint/astro-integration-issues/pages/IssueDetail.astro',
				});

				if (options?.slotRegistry) {
					options.slotRegistry.repoTabs.push({
						id: 'issues',
						label: 'Issues',
						componentPath: '@forgepoint/astro-integration-issues/components/IssuesTab.vue',
						order: 10,
					});
				}
			},
		},
	};
}
