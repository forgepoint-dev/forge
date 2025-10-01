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

export type ActionScope = 'dashboard' | 'repository';

export interface ActionContext {
	scope: ActionScope;
	repository?: RepositoryContext;
	navigate: (path: string) => void;
}

export type ActionHandler = (context: ActionContext) => void | Promise<void>;

export interface ActionSlot {
	id: string;
	label: string;
	scope: ActionScope;
	order?: number;
	kind: 'link' | 'handler';
	href?: string;
	handler?: ActionHandler;
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
