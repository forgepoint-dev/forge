<script setup lang="ts">
import { computed, reactive, ref, watch, onMounted } from 'vue';
import { createIssue, getRepositoryByPath } from '../lib/client';

const props = defineProps<{
	repositoryId?: string;
	repositoryPath?: string;
}>();

const form = reactive({
	repositoryPath: props.repositoryPath ?? '',
	title: '',
	description: '',
});

const submitting = ref(false);
const error = ref<string | null>(null);
const success = ref<string | null>(null);
const resolvingRepository = ref(false);
const resolvedRepositoryId = ref<string | null>(props.repositoryId ?? null);
const lastResolvedPath = ref<string | null>(props.repositoryPath ?? null);

const repositoryPathLabel = computed(() => {
	if (props.repositoryId) {
		return props.repositoryPath ?? 'Repository';
	}
	return form.repositoryPath;
});

watch(
	() => props.repositoryId,
	(newId) => {
		resolvedRepositoryId.value = newId ?? null;
		if (newId) {
			lastResolvedPath.value = props.repositoryPath ?? null;
		}
	}
);

watch(
	() => form.repositoryPath,
	() => {
		if (!props.repositoryId) {
			resolvedRepositoryId.value = null;
		}
	}
);

async function ensureRepositoryId(): Promise<string> {
	if (props.repositoryId) {
		return props.repositoryId;
	}

	const path = form.repositoryPath.trim();
	if (!path) {
		throw new Error('Repository path is required. Example: team/my-repo');
	}

	let sanitized = path.replace(/^\/+|\/+$/g, '');

	if (resolvedRepositoryId.value && lastResolvedPath.value === sanitized) {
		return resolvedRepositoryId.value;
	}

	resolvingRepository.value = true;
	try {
		const response = await getRepositoryByPath(sanitized);
		const repo = response.getRepository;
		if (!repo) {
			throw new Error(`Repository “${path}” was not found.`);
		}
		resolvedRepositoryId.value = repo.id;
		lastResolvedPath.value = sanitized;
		form.repositoryPath = sanitized;
		return repo.id;
	} finally {
		resolvingRepository.value = false;
	}
}

async function submit() {
	error.value = null;
	success.value = null;

	if (!form.title.trim()) {
		error.value = 'Title is required.';
		return;
	}

	let repositoryId: string;
	try {
		repositoryId = await ensureRepositoryId();
	} catch (err) {
		error.value = err instanceof Error ? err.message : 'Unable to locate repository.';
		return;
	}

	submitting.value = true;
	try {
		const response = await createIssue(repositoryId, {
			title: form.title.trim(),
			description: form.description.trim() || undefined,
		});

		success.value = `Issue “${response.createIssue.title}” created.`;
		form.title = '';
		form.description = '';

		const issueNumber = response.createIssue.number;
		const targetRepoId = response.createIssue.repositoryId;
		const pathForRedirect = (props.repositoryPath && props.repositoryPath.trim())
			|| lastResolvedPath.value
			|| form.repositoryPath.trim();

		if (typeof window !== 'undefined') {
			if (pathForRedirect) {
				const sanitized = pathForRedirect.replace(/^\/+|\/+$/g, '');
				window.location.href = `/${sanitized}/issues/${issueNumber}`;
			} else {
				const params = new URLSearchParams({ repositoryId: targetRepoId, issueNumber: String(issueNumber) });
				window.location.href = `/issues/${response.createIssue.id}?${params.toString()}`;
			}
		}
	} catch (err) {
		error.value = err instanceof Error ? err.message : 'Failed to create issue.';
	} finally {
		submitting.value = false;
	}
}

onMounted(async () => {
	if (!props.repositoryId && props.repositoryPath) {
		try {
			await ensureRepositoryId();
		} catch (err) {
			console.error(err);
		}
	}
});

function cancel() {
	if (typeof window !== 'undefined') {
		window.history.back();
	}
}
</script>

<template>
	<form class="space-y-6" @submit.prevent="submit">
		<section class="space-y-3">
			<h2 class="text-base font-semibold text-foreground">Repository</h2>
			<div v-if="props.repositoryId" class="rounded-md border border-border bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
				Issues will be created in
				<span class="font-medium">{{ repositoryPathLabel || 'the selected repository' }}</span>.
			</div>
			<div v-else class="space-y-2">
				<label for="repository-path" class="text-sm font-medium text-foreground">
					Repository Path
				</label>
				<input
					id="repository-path"
					v-model="form.repositoryPath"
					type="text"
					placeholder="group/my-repo"
					class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/40"
					:disabled="resolvingRepository || submitting"
				/>
				<p class="text-xs text-muted-foreground">Provide the repository path as it appears in the browser (group/subgroup/repo).</p>
				<p v-if="resolvingRepository" class="text-xs text-muted-foreground">Validating repository…</p>
			</div>
		</section>

		<section class="space-y-4">
			<div class="space-y-2">
				<label for="issue-title" class="text-sm font-medium text-foreground">
					Title
					<span class="text-red-500">*</span>
				</label>
				<input
					id="issue-title"
					v-model="form.title"
					type="text"
					class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/40"
					maxlength="200"
					:disabled="submitting"
					placeholder="Fix login bug"
					required
				/>
			</div>

			<div class="space-y-2">
				<label for="issue-description" class="text-sm font-medium text-foreground">Description</label>
				<textarea
					id="issue-description"
					v-model="form.description"
					class="w-full min-h-[160px] rounded-md border border-input bg-background px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/40"
					placeholder="Provide context, steps to reproduce, or acceptance criteria."
					:disabled="submitting"
				></textarea>
				<p class="text-xs text-muted-foreground">Markdown is supported.</p>
			</div>
		</section>

		<div class="space-y-2">
			<p v-if="error" class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
				{{ error }}
			</p>
			<p v-if="success" class="rounded-md border border-emerald-500/40 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-200">
				{{ success }}
			</p>
		</div>

		<div class="flex items-center justify-end gap-2">
			<button
				type="button"
				class="rounded-md border border-input px-3 py-2 text-sm text-muted-foreground transition hover:bg-accent hover:text-accent-foreground"
				:disabled="submitting"
				@click="cancel"
			>
				Cancel
			</button>
			<button
				type="submit"
				class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm transition hover:bg-primary/90 disabled:opacity-60"
				:disabled="submitting || resolvingRepository"
			>
				{{ submitting ? 'Creating…' : 'Create Issue' }}
			</button>
		</div>
	</form>
</template>
