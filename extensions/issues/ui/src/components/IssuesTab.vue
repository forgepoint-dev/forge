<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { getIssuesForRepository, type Issue } from '../lib/client';

const props = defineProps<{
	repository: {
		id: string;
		slug: string;
		fullPath: string;
		isRemote: boolean;
		remoteUrl: string | null;
	};
}>();

const issues = ref<Issue[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

async function fetchIssues() {
	loading.value = true;
	error.value = null;

	try {
		const response = await getIssuesForRepository(props.repository.id);
		issues.value = response.getIssuesForRepository;
	} catch (err) {
		error.value = err instanceof Error ? err.message : 'Failed to load issues';
		issues.value = [];
	} finally {
		loading.value = false;
	}
}

onMounted(fetchIssues);

watch(
	() => props.repository.id,
	() => {
		fetchIssues();
	}
);

function getStatusColor(status: Issue['status']) {
	switch (status) {
		case 'OPEN':
			return 'bg-green-100 text-green-800 border-green-200';
		case 'IN_PROGRESS':
			return 'bg-blue-100 text-blue-800 border-blue-200';
		case 'CLOSED':
			return 'bg-gray-100 text-gray-800 border-gray-200';
	}
}

function getStatusLabel(status: Issue['status']) {
	switch (status) {
		case 'OPEN':
			return 'Open';
		case 'IN_PROGRESS':
			return 'In Progress';
		case 'CLOSED':
			return 'Closed';
	}
}
</script>

<template>
	<div class="space-y-4">
		<div class="flex items-center justify-between">
			<h3 class="text-lg font-semibold">Issues</h3>
			<button
				class="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
			>
				New Issue
			</button>
		</div>

		<div v-if="loading" class="rounded-lg border p-6 text-sm text-muted-foreground bg-muted/40">
			Loading issues…
		</div>

		<div
			v-else-if="error"
			class="rounded-lg border border-destructive/50 bg-destructive/10 p-6 text-sm text-destructive"
		>
			{{ error }}
		</div>

		<div v-else-if="issues.length === 0" class="rounded-lg border p-6 text-center text-sm text-muted-foreground">
			No issues found for this repository.
		</div>

		<div v-else class="space-y-2">
			<div
				v-for="issue in issues"
				:key="issue.id"
				class="rounded-lg border p-4 hover:bg-muted/50 transition"
			>
				<div class="flex items-start justify-between gap-3">
					<div class="flex-1">
						<h4 class="font-medium">{{ issue.title }}</h4>
						<p class="text-xs text-muted-foreground mt-1">
							{{ issue.id }} · Created {{ new Date(issue.createdAt).toLocaleDateString() }}
						</p>
					</div>
					<span
						:class="getStatusColor(issue.status)"
						class="inline-flex items-center rounded border px-2 py-0.5 text-xs font-medium"
					>
						{{ getStatusLabel(issue.status) }}
					</span>
				</div>
			</div>
		</div>
	</div>
</template>
