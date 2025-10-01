<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { getIssuesForRepository, getRepositoryByPath, type Issue } from '../lib/client';

const props = defineProps<{
	repositoryId?: string;
	repositoryPath?: string;
}>();

const issues = ref<Issue[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const resolvedRepositoryId = ref<string | null>(props.repositoryId ?? null);

async function fetchIssues(repositoryId: string) {
	loading.value = true;
	error.value = null;

	try {
		const response = await getIssuesForRepository(repositoryId);
		issues.value = response.getIssuesForRepository;
	} catch (e) {
		error.value = e instanceof Error ? e.message : 'Failed to load issues';
		issues.value = [];
	} finally {
		loading.value = false;
	}
}

function resetState() {
	issues.value = [];
	loading.value = false;
	error.value = null;
	resolvedRepositoryId.value = null;
}

async function ensureRepositoryId(): Promise<string | null> {
	if (props.repositoryId) {
		resolvedRepositoryId.value = props.repositoryId;
		return props.repositoryId;
	}

	const rawPath = props.repositoryPath?.trim();
	const path = rawPath ? rawPath.replace(/^\/+|\/+$/g, '') : rawPath;
	if (!path) {
		resolvedRepositoryId.value = null;
		return null;
	}

	try {
		const response = await getRepositoryByPath(path);
		const repository = response.getRepository;
		if (repository) {
			resolvedRepositoryId.value = repository.id;
			return repository.id;
		}
		resolvedRepositoryId.value = null;
		error.value = `Repository "${path}" was not found.`;
		return null;
	} catch (err) {
		resolvedRepositoryId.value = null;
		error.value = err instanceof Error ? err.message : 'Failed to resolve repository';
		return null;
	}
}

async function loadIssues() {
	loading.value = true;
	error.value = null;

	const repositoryId = await ensureRepositoryId();
	if (!repositoryId) {
		resetState();
		loading.value = false;
		return;
	}

	await fetchIssues(repositoryId);
}

onMounted(loadIssues);

watch(
	() => [props.repositoryId, props.repositoryPath],
	() => {
		loadIssues();
	}
);
</script>

<template>
	<div class="space-y-4 text-sm">
		<h2 class="text-2xl font-semibold text-foreground">Issues</h2>
		
		<div v-if="loading" class="text-muted-foreground">Loading issues…</div>
		
		<div
			v-else-if="error"
			class="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-destructive"
		>
			{{ error }}
		</div>

		<div v-else-if="!props.repositoryId" class="rounded-md border border-border bg-muted/40 px-4 py-3 text-muted-foreground">
			Select a repository to view issues.
		</div>
		
		<div v-else-if="issues.length === 0" class="rounded-md border border-border bg-muted/40 px-4 py-3 text-muted-foreground">
			No issues found for this repository.
		</div>
		
		<ul v-else class="space-y-2">
			<li
				v-for="issue in issues"
				:key="issue.id"
				class="rounded-lg border border-border bg-card transition hover:bg-accent/40"
			>
				<a
					:href="
						props.repositoryPath
							? `/${props.repositoryPath}/issues/${issue.number}`
							: props.repositoryId
								? `/issues/${issue.id}?repositoryId=${encodeURIComponent(props.repositoryId)}`
								: `/issues/${issue.id}`
					"
					class="block px-4 py-3"
				>
					<div class="flex items-center justify-between gap-3">
						<h3 class="text-base font-semibold text-foreground hover:text-primary">
							#{{ issue.number }} · {{ issue.title }}
						</h3>
						<span
							class="inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium"
							:class="{
								'border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300': issue.status === 'OPEN',
								'border-slate-500/40 bg-slate-500/10 text-slate-700 dark:text-slate-300': issue.status === 'CLOSED',
								'border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300': issue.status === 'IN_PROGRESS'
							}"
						>
							{{ issue.status.replace('_', ' ') }}
						</span>
					</div>
					<p v-if="issue.description" class="mt-2 text-sm text-muted-foreground">
						{{ issue.description }}
					</p>
					<p class="mt-2 text-xs text-muted-foreground">
						Created: {{ new Date(issue.createdAt).toLocaleDateString() }}
					</p>
				</a>
			</li>
		</ul>
	</div>
</template>
