<script setup lang="ts">
import { ref, computed, onErrorCaptured } from 'vue';
import { repoTabs } from 'virtual:forge/slots/repo-tabs';
import type { RepositoryContext } from '../lib/slots';

const props = defineProps<{
	repository: RepositoryContext;
}>();

const componentError = ref<string | null>(null);

// Default tab is README when available; parent component decides whether to provide slot content.
const activeTab = ref<string>('readme');

const tabs = computed(() => [
	{ id: 'readme', label: 'README' },
	{ id: 'files', label: 'Files' },
	...repoTabs.map((slot) => ({ id: slot.id, label: slot.label })),
]);

const activeSlot = computed(() => {
	return repoTabs.find((slot) => slot.id === activeTab.value);
});

onErrorCaptured((err) => {
	console.error('[ExtensionTabs] Component error:', err);
	componentError.value = err instanceof Error ? err.message : 'Extension component failed to render';
	return false;
});
</script>

<template>
	<div class="space-y-4">
		<div class="border-b">
			<nav class="flex gap-6">
				<button
					v-for="tab in tabs"
					:key="tab.id"
					@click="activeTab = tab.id; componentError = null"
					:class="[
						'pb-3 text-sm font-medium border-b-2 transition',
						activeTab === tab.id
							? 'border-primary text-foreground'
							: 'border-transparent text-muted-foreground hover:text-foreground hover:border-muted-foreground',
					]"
				>
					{{ tab.label }}
				</button>
			</nav>
		</div>

		<div v-if="componentError" class="p-4 border border-red-300 bg-red-50 rounded">
			<h3 class="text-sm font-semibold text-red-800 mb-1">Extension Error</h3>
			<p class="text-sm text-red-700">{{ componentError }}</p>
			<button
				@click="componentError = null; activeTab = 'files'"
				class="mt-2 text-xs text-red-800 underline hover:no-underline"
			>
				Return to Files tab
			</button>
		</div>

		<div v-else-if="activeTab === 'readme'">
			<slot name="readme" />
		</div>

		<div v-else-if="activeTab === 'files'">
			<slot name="files" />
		</div>

		<div v-else-if="activeSlot">
			<component :is="activeSlot.component" :repository="repository" />
		</div>

		<div v-else class="p-4 text-muted-foreground">
			No content available for this tab.
		</div>
	</div>
</template>
