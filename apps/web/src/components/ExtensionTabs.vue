<script setup lang="ts">
import { ref, computed } from 'vue';
import { repoTabs } from 'virtual:forge/slots/repo-tabs';
import type { RepositoryContext } from '../lib/slots';

const props = defineProps<{
	repository: RepositoryContext;
}>();

const activeTab = ref<string>(repoTabs[0]?.id || 'files');

const tabs = computed(() => [
	{ id: 'files', label: 'Files' },
	...repoTabs.map((slot) => ({ id: slot.id, label: slot.label })),
]);

const activeSlot = computed(() => {
	return repoTabs.find((slot) => slot.id === activeTab.value);
});
</script>

<template>
	<div class="space-y-4">
		<div class="border-b">
			<nav class="flex gap-6">
				<button
					v-for="tab in tabs"
					:key="tab.id"
					@click="activeTab = tab.id"
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

		<div v-if="activeTab === 'files'">
			<slot name="files" />
		</div>

		<div v-else-if="activeSlot">
			<component :is="activeSlot.component" :repository="repository" />
		</div>
	</div>
</template>
