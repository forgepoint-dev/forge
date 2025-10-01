<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { getIssuesForRepository, type Issue } from '../lib/client';

const props = defineProps<{
	repositoryId?: string;
}>();

const issues = ref<Issue[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

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
}

onMounted(() => {
	if (props.repositoryId) {
		fetchIssues(props.repositoryId);
	} else {
		resetState();
	}
});

watch(
	() => props.repositoryId,
	(newRepositoryId) => {
		if (newRepositoryId) {
			fetchIssues(newRepositoryId);
		} else {
			resetState();
		}
	}
);
</script>

<template>
	<div class="issues-list">
		<h2 class="text-2xl font-bold mb-4">Issues</h2>
		
		<div v-if="loading" class="text-gray-600">
			Loading issues...
		</div>
		
		<div v-else-if="error" class="text-red-600 bg-red-50 p-4 rounded">
			{{ error }}
		</div>

		<div v-else-if="!props.repositoryId" class="text-gray-600">
			Select a repository to view issues.
		</div>
		
		<div v-else-if="issues.length === 0" class="text-gray-600">
			No issues found.
		</div>
		
		<ul v-else class="space-y-2">
			<li 
				v-for="issue in issues" 
				:key="issue.id"
				class="border border-gray-200 rounded-lg p-4 hover:bg-gray-50 transition-colors"
			>
				<a 
					:href="`/issues/${issue.id}`"
					class="block"
				>
					<div class="flex items-center justify-between">
						<h3 class="text-lg font-semibold text-blue-600 hover:text-blue-800">
							{{ issue.title }}
						</h3>
						<span 
							class="px-2 py-1 rounded text-sm"
							:class="{
								'bg-green-100 text-green-800': issue.status === 'OPEN',
								'bg-gray-100 text-gray-800': issue.status === 'CLOSED',
								'bg-yellow-100 text-yellow-800': issue.status === 'IN_PROGRESS'
							}"
						>
							{{ issue.status }}
						</span>
					</div>
					<p v-if="issue.description" class="text-gray-600 mt-2">
						{{ issue.description }}
					</p>
					<p class="text-sm text-gray-400 mt-2">
						Created: {{ new Date(issue.createdAt).toLocaleDateString() }}
					</p>
				</a>
			</li>
		</ul>
	</div>
</template>
