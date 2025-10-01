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
	},
);

function getStatusColor(status: Issue['status']) {
	switch (status) {
		case 'OPEN':
			return 'border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300';
		case 'IN_PROGRESS':
			return 'border-sky-500/40 bg-sky-500/10 text-sky-700 dark:text-sky-300';
		case 'CLOSED':
			return 'border-slate-500/40 bg-slate-500/10 text-slate-700 dark:text-slate-300';
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
			<h3 class="text-lg font-semibold text-foreground">Issues</h3>
			<a
				:href="`/${props.repository.fullPath}/issues`"
				class="text-sm font-medium text-primary underline-offset-4 hover:underline"
			>
				View all
			</a>
		</div>

		<div v-if="loading" class="rounded-lg border border-border bg-muted/40 p-6 text-sm text-muted-foreground">
			Loading issues…
		</div>

		<div
			v-else-if="error"
			class="rounded-lg border border-destructive/40 bg-destructive/10 p-6 text-sm text-destructive"
		>
			{{ error }}
		</div>

		<div v-else-if="issues.length === 0" class="rounded-lg border border-border bg-muted/40 p-6 text-center text-sm text-muted-foreground">
			No issues found for this repository.
		</div>

		<div v-else class="space-y-2">
			<a
				v-for="issue in issues"
				:key="issue.id"
				:href="`/${props.repository.fullPath}/issues/${issue.number}`"
				class="flex items-start justify-between gap-3 rounded-lg border border-border bg-card p-4 transition hover:bg-accent/50"
			>
				<div class="flex-1">
					<h4 class="font-medium text-foreground">#{{ issue.number }} · {{ issue.title }}</h4>
					<p class="mt-1 text-xs text-muted-foreground">
						Created {{ new Date(issue.createdAt).toLocaleDateString() }}
					</p>
				</div>
				<span
					:class="getStatusColor(issue.status)"
					class="inline-flex items-center rounded border px-2 py-0.5 text-xs font-medium"
				>
					{{ getStatusLabel(issue.status) }}
				</span>
			</a>
		</div>
	</div>
</template>
