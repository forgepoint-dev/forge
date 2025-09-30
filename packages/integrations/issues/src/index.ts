import type { AstroIntegration } from 'astro';

export default function issuesIntegration(): AstroIntegration {
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
			},
		},
	};
}
