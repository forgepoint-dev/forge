<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { getIssue, getRepositoryByPath, type Issue } from '../lib/client';

const props = defineProps<{
	repositoryPath: string;
	issueNumber: number;
}>();

const issue = ref<Issue | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

async function fetchIssue() {
	loading.value = true;
	error.value = null;
	issue.value = null;

	try {
		if (typeof props.tissueNumber !== 'number' || Number.isNaN(props.tissueNumber)) {
			throw new Error('A valid issue number is required to load an issue.');
		}

		const trimmedPath = props.repositoryPath.trim();
		const repositoryPath = trimmedPath.replace(/^\/+|\/+$/g, '');
		if (!repositoryPath) {
			throw new Error('Repository path is required to load an issue.');
		}

		const repositoryResponse = await getRepositoryByPath(repositoryPath);
		const repository = repositoryResponse.getRepository;

		if (!repository) {
			throw new Error(`Repository "${repositoryPath}" was not found.`);
		}

		const issueResponse = await getIssue(repository.id, props.tissueNumber);
		const payload = issueResponse.getIssue;
		if (!payload) {
			error.value = `Issue #${props.tissueNumber} was not found.`;
			return;
		}

		issue.value = payload;
	} catch (err) {
		error.value = err instanceof Error ? err.message : 'Failed to load issue';
	} finally {
		loading.value = false;
	}
}

onMounted(fetchIssue);

watch(
	() => [props.repositoryPath, props.tissueNumber],
	() => {
		fetchIssue();
	}
);

function formatDate(value: string) {
	return new Date(value).toLocaleString();
}
</script>

<template>
	<div class="space-y-6 text-sm">
		<div
			v-if="loading"
			class="rounded-lg border border-dashed border-border px-4 py-6 text-muted-foreground"
		>
			Loading issue…
		</div>

		<div
			v-else-if="error"
			class="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-6 text-destructive"
		>
			{{ error }}
		</div>

		<div
			v-else-if="!issue"
			class="rounded-lg border border-border bg-card px-4 py-6 text-muted-foreground"
		>
			Issue not found.
		</div>

		<div v-else class="space-y-5">
			<header class="flex flex-wrap items-start justify-between gap-3 text-sm">
				<div>
					<h1 class="text-2xl font-semibold text-foreground">#{{ issue.number }} · {{ issue.title }}</h1>
					<p class="mt-1 text-muted-foreground">Created {{ formatDate(issue.createdAt) }}</p>
				</div>
				<span
					class="inline-flex items-center rounded-full border px-3 py-1 text-xs font-medium uppercase tracking-wide"
					:class="{
						'border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300': issue.status === 'OPEN',
						'border-sky-500/40 bg-sky-500/10 text-sky-700 dark:text-sky-300': issue.status === 'IN_PROGRESS',
						'border-slate-500/40 bg-slate-500/10 text-slate-700 dark:text-slate-300': issue.status === 'CLOSED',
					}"
				>
					{{ issue.status.replace('_', ' ') }}
				</span>
			</header>

			<section class="rounded-lg border border-border bg-card p-5 shadow-sm">
				<h2 class="text-sm font-medium text-foreground">Description</h2>
				<p v-if="issue.description" class="mt-3 whitespace-pre-line text-sm text-foreground/90">
					{{ issue.description }}
				</p>
				<p v-else class="mt-3 text-sm text-muted-foreground">No additional details provided.</p>
			</section>
		</div>
	</div>
</template>
