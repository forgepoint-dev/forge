declare module 'virtual:forge/slots/repo-tabs' {
	import type { Component } from 'vue';

	export interface RepositoryTabSlot {
		id: string;
		label: string;
		component: Component;
		order: number;
	}

	export const repoTabs: RepositoryTabSlot[];
}

declare module 'virtual:forge/slots/group-tabs' {
	import type { Component } from 'vue';

	export interface GroupTabSlot {
		id: string;
		label: string;
		component: Component;
		order: number;
	}

	export const groupTabs: GroupTabSlot[];
}

declare module 'virtual:forge/slots/homepage-widgets' {
	import type { Component } from 'vue';

	export interface HomepageWidgetSlot {
		id: string;
		component: Component;
		order: number;
	}

	export const homepageWidgets: HomepageWidgetSlot[];
}

declare module 'virtual:forge/slots/actions' {
	import type { ActionSlot } from './slots';

	export const actionSlots: ActionSlot[];
}
