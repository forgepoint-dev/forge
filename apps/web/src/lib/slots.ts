import type { Component } from 'vue';

export interface RepositoryTabSlot {
	id: string;
	label: string;
	component: Component;
	order?: number;
}

export interface GroupTabSlot {
	id: string;
	label: string;
	component: Component;
	order?: number;
}

export interface HomepageWidgetSlot {
	id: string;
	component: Component;
	order?: number;
}

export interface RepositoryContext {
	id: string;
	slug: string;
	fullPath: string;
	isRemote: boolean;
	remoteUrl: string | null;
}

export interface GroupContext {
	id: string;
	slug: string;
	fullPath: string;
}
