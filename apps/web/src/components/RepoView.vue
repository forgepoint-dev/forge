<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import UiButton from './ui/button.vue'
import UiInput from './ui/input.vue'
import { graphqlRequest } from '../lib/graphql'

const props = defineProps<{ fullPath: string }>()

type RepositoryDetails = {
  id: string
  slug: string
  group: { id: string; slug: string } | null
  isRemote: boolean
  remoteUrl?: string | null
}

type GroupDetails = {
  id: string
  slug: string
  parent: { id: string; slug: string } | null
  repositories: { id: string; slug: string; isRemote: boolean; remoteUrl?: string | null }[]
}

const segments = computed(() => props.fullPath.split('/').filter(Boolean))
const repoName = computed(() => segments.value[segments.value.length - 1] || '')
const groups = computed(() => segments.value.slice(0, -1))
const badge = computed(() => (groups.value[0]?.[0] || repoName.value[0] || 'F').toUpperCase())

const repository = ref<RepositoryDetails | null>(null)
const groupDetails = ref<GroupDetails | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)

const isRemote = computed(() => repository.value?.isRemote ?? false)
const remoteUrl = computed(() => repository.value?.remoteUrl ?? null)

const siblingRepositories = computed(() => {
  if (!groupDetails.value) return []
  return groupDetails.value.repositories.filter((repo) => repo.id !== repository.value?.id)
})

async function loadData(path: string) {
  loading.value = true
  error.value = null
  repository.value = null
  groupDetails.value = null

  try {
    const repoResponse = await graphqlRequest<{ getRepository: RepositoryDetails | null }>({
      query: /* GraphQL */ `
        query RepositoryByPath($path: String!) {
          getRepository(path: $path) {
            id
            slug
            isRemote
            remoteUrl
            group {
              id
              slug
            }
          }
        }
      `,
      variables: { path },
    })

    if (!repoResponse.getRepository) {
      error.value = 'Repository not found.'
      return
    }

    repository.value = repoResponse.getRepository

    if (groups.value.length > 0) {
      const groupPath = groups.value.join('/')
      const groupResponse = await graphqlRequest<{ getGroup: GroupDetails | null }>({
        query: /* GraphQL */ `
          query GroupByPath($path: String!) {
            getGroup(path: $path) {
              id
              slug
              parent {
                id
                slug
              }
              repositories {
                id
                slug
                isRemote
                remoteUrl
              }
            }
          }
        `,
        variables: { path: groupPath },
      })

      groupDetails.value = groupResponse.getGroup
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to load repository'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  loadData(props.fullPath)
})

watch(
  () => props.fullPath,
  (nextPath, prevPath) => {
    if (nextPath !== prevPath) {
      loadData(nextPath)
    }
  },
)

// Placeholder values until repo metadata expands
const branches = ['main']
const branch = ref('main')

</script>

<template>
  <div class="min-h-screen">
    <!-- Repo header -->
    <div class="border-b bg-background/60 backdrop-blur supports-[backdrop-filter]:bg-background/40">
      <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-6">
        <div class="flex items-center gap-3">
          <div class="size-8 rounded bg-primary/10 text-primary grid place-items-center font-black">{{ badge }}</div>
          <div class="text-xl">
            <template v-if="groups.length">
              <span class="text-muted-foreground">{{ groups.join('/') }}</span>
              <span class="mx-1">/</span>
            </template>
            <span class="font-semibold">{{ repoName }}</span>
          </div>
          <span
            v-if="isRemote"
            class="ml-2 inline-flex items-center rounded border px-2 py-0.5 text-xs text-amber-700 border-amber-200 bg-amber-100/70"
          >Remote · read-only</span>
          <span v-else class="ml-2 inline-flex items-center rounded border px-2 py-0.5 text-xs text-muted-foreground">
            Public
          </span>
          <div class="ml-auto flex items-center gap-2">
            <UiButton variant="outline">Open in IDE</UiButton>
            <UiButton>Clone</UiButton>
          </div>
        </div>
        <p v-if="repository" class="mt-4 text-sm text-muted-foreground space-y-1">
          <span class="block">
            Repository ID: <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ repository.id }}</code>
          </span>
          <span v-if="isRemote && remoteUrl" class="block">
            Linked remote: <a :href="remoteUrl" class="underline hover:text-foreground" target="_blank" rel="noreferrer">{{ remoteUrl }}</a>
          </span>
        </p>
      </div>
    </div>

    <!-- Content -->
    <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-8">
      <div v-if="loading" class="rounded-lg border p-6 text-sm text-muted-foreground bg-muted/40">
        Loading repository…
      </div>
      <div v-else-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 p-6 text-sm text-destructive">
        {{ error }}
      </div>
      <div v-else-if="!repository" class="rounded-lg border p-6 text-sm text-muted-foreground bg-muted/40">
        Repository not found.
      </div>
      <div v-else class="space-y-6">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div class="flex items-center gap-2">
            <label class="text-xs text-muted-foreground">Branch</label>
            <select v-model="branch" class="h-9 rounded-md border border-input bg-background px-2 text-sm">
              <option v-for="b in branches" :key="b" :value="b">{{ b }}</option>
            </select>
          </div>
          <div class="flex items-center gap-2 w-full sm:w-80">
            <UiInput placeholder="Search this repo" />
            <UiButton variant="outline">Go</UiButton>
          </div>
        </div>

        <section class="rounded-lg border p-5 bg-card">
          <h3 class="text-sm font-semibold text-muted-foreground">Repository Details</h3>
          <dl class="mt-3 space-y-2 text-sm">
            <div class="flex items-center justify-between gap-4">
              <dt class="text-muted-foreground">Repository ID</dt>
              <dd><code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ repository.id }}</code></dd>
            </div>
            <div v-if="groups.length" class="flex items-center justify-between gap-4">
              <dt class="text-muted-foreground">Group</dt>
              <dd class="font-medium">{{ groups.join('/') }}</dd>
            </div>
            <div v-if="isRemote && remoteUrl" class="flex items-center justify-between gap-4">
              <dt class="text-muted-foreground">Remote URL</dt>
              <dd class="text-right break-all">
                <a :href="remoteUrl" class="underline hover:text-foreground" target="_blank" rel="noreferrer">{{ remoteUrl }}</a>
              </dd>
            </div>
          </dl>
        </section>

        <section v-if="groupDetails" class="rounded-lg border p-5">
          <h3 class="text-sm font-semibold text-muted-foreground">Other repositories in this group</h3>
          <ul class="mt-3 space-y-2 text-sm text-muted-foreground">
            <li v-if="siblingRepositories.length === 0">No other repositories in this group.</li>
            <li v-else v-for="repo in siblingRepositories" :key="repo.id" class="flex items-center gap-2">
              <a :href="`/${[...groups, repo.slug].join('/')}`" class="text-foreground hover:underline flex-1">
                {{ [...groups, repo.slug].join('/') }}
              </a>
              <span v-if="repo.isRemote" class="inline-flex items-center rounded border px-2 py-0.5 text-[10px] uppercase tracking-wide text-amber-700 border-amber-200 bg-amber-100/70">
                Remote
              </span>
            </li>
          </ul>
        </section>
      </div>
    </div>
  </div>
</template>
