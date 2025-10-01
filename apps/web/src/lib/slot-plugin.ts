import type { Plugin } from 'vite';

export interface SlotDefinition {
	id: string;
	label: string;
	componentPath: string;
	order?: number;
}

interface SlotRegistry {
	repoTabs: SlotDefinition[];
	groupTabs: SlotDefinition[];
	homepageWidgets: SlotDefinition[];
}

export function createSlotRegistry(): SlotRegistry {
	return {
		repoTabs: [],
		groupTabs: [],
		homepageWidgets: [],
	};
}

export function createSlotPlugin(registry: SlotRegistry): Plugin & { __registry: SlotRegistry } {
	const virtualModuleIds = {
		repoTabs: 'virtual:forge/slots/repo-tabs',
		groupTabs: 'virtual:forge/slots/group-tabs',
		homepageWidgets: 'virtual:forge/slots/homepage-widgets',
	};

	const resolvedModuleIds = {
		repoTabs: '\0' + virtualModuleIds.repoTabs,
		groupTabs: '\0' + virtualModuleIds.groupTabs,
		homepageWidgets: '\0' + virtualModuleIds.homepageWidgets,
	};

	return {
		name: 'forge-slot-plugin',
		__registry: registry,
		resolveId(id) {
			if (id === virtualModuleIds.repoTabs) return resolvedModuleIds.repoTabs;
			if (id === virtualModuleIds.groupTabs) return resolvedModuleIds.groupTabs;
			if (id === virtualModuleIds.homepageWidgets) return resolvedModuleIds.homepageWidgets;
		},
		load(id) {
			if (id === resolvedModuleIds.repoTabs) {
				const sorted = [...registry.repoTabs].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));

				// Validate slot registrations
				const seenIds = new Set<string>();
				const duplicates = sorted.filter(slot => {
					if (seenIds.has(slot.id)) return true;
					seenIds.add(slot.id);
					return false;
				});

				if (duplicates.length > 0) {
					console.warn(
						'[Forge Slots] Duplicate slot IDs detected in repo-tabs:',
						duplicates.map(s => s.id).join(', ')
					);
				}

				const imports = sorted.map((slot, idx) => `import Component${idx} from '${slot.componentPath}';`).join('\n');
				const slots = sorted
					.map(
						(slot, idx) => `{
  id: '${slot.id}',
  label: '${slot.label}',
  component: Component${idx},
  order: ${slot.order ?? 0}
}`,
					)
					.join(',\n  ');
				return `${imports}\n\nexport const repoTabs = [\n  ${slots}\n];`;
			}
			if (id === resolvedModuleIds.groupTabs) {
				const sorted = [...registry.groupTabs].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));

				// Validate slot registrations
				const seenIds = new Set<string>();
				const duplicates = sorted.filter(slot => {
					if (seenIds.has(slot.id)) return true;
					seenIds.add(slot.id);
					return false;
				});

				if (duplicates.length > 0) {
					console.warn(
						'[Forge Slots] Duplicate slot IDs detected in group-tabs:',
						duplicates.map(s => s.id).join(', ')
					);
				}

				const imports = sorted.map((slot, idx) => `import Component${idx} from '${slot.componentPath}';`).join('\n');
				const slots = sorted
					.map(
						(slot, idx) => `{
  id: '${slot.id}',
  label: '${slot.label}',
  component: Component${idx},
  order: ${slot.order ?? 0}
}`,
					)
					.join(',\n  ');
				return `${imports}\n\nexport const groupTabs = [\n  ${slots}\n];`;
			}
			if (id === resolvedModuleIds.homepageWidgets) {
				const sorted = [...registry.homepageWidgets].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));

				// Validate slot registrations
				const seenIds = new Set<string>();
				const duplicates = sorted.filter(slot => {
					if (seenIds.has(slot.id)) return true;
					seenIds.add(slot.id);
					return false;
				});

				if (duplicates.length > 0) {
					console.warn(
						'[Forge Slots] Duplicate slot IDs detected in homepage-widgets:',
						duplicates.map(s => s.id).join(', ')
					);
				}

				const imports = sorted.map((slot, idx) => `import Component${idx} from '${slot.componentPath}';`).join('\n');
				const slots = sorted
					.map(
						(slot, idx) => `{
  id: '${slot.id}',
  component: Component${idx},
  order: ${slot.order ?? 0}
}`,
					)
					.join(',\n  ');
				return `${imports}\n\nexport const homepageWidgets = [\n  ${slots}\n];`;
			}
		},
	};
}
