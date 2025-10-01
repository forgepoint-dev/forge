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
	version: 1;
	id: string;
	slug: string;
	fullPath: string;
	isRemote: boolean;
	remoteUrl: string | null;
}

export interface GroupContext {
	version: 1;
	id: string;
	slug: string;
	fullPath: string;
}
