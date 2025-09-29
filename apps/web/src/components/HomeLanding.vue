<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiInput from './ui/input.vue'
import { graphqlRequest } from '../lib/graphql'
import { CreateRepositoryDialog, LinkRepositoryDialog, Button } from 'design'

type GroupNode = {
  id: string
  slug: string
  parent: { id: string; slug: string } | null
}

type RepositoryNode = {
  id: string
  slug: string
  group: { id: string; slug: string } | null
  isRemote: boolean
}

type RepoCard = {
  id: string
  path: string
  isRemote: boolean
}

const repos = ref<RepoCard[]>([])
const groups = ref<GroupNode[]>([])
const loading = ref(true)
const error = ref<string | null>(null)

const showCreateDialog = ref(false)
const showLinkDialog = ref(false)

const hasRepos = computed(() => repos.value.length > 0)

function buildGroupPath(groups: Map<string, { slug: string; parentId: string | null }>, groupId: string | null) {
  if (!groupId) return []

  const path: string[] = []
  let current: string | null = groupId

  while (current) {
    const entry = groups.get(current)
    if (!entry) break
    path.push(entry.slug)
    current = entry.parentId
  }

  return path.reverse()
}

async function loadData() {
  try {
    loading.value = true
    error.value = null
    const query = /* GraphQL */ `
      query HomeLandingData {
        getAllGroups {
          id
          slug
          parent {
            id
            slug
          }
        }
        getAllRepositories {
          id
          slug
          isRemote
          group {
            id
            slug
          }
        }
      }
    `

    const data = await graphqlRequest<{ getAllGroups: GroupNode[]; getAllRepositories: RepositoryNode[] }>({
      query,
    })

    groups.value = data.getAllGroups

    const groupMap = new Map<string, { slug: string; parentId: string | null }>()
    for (const group of data.getAllGroups) {
      groupMap.set(group.id, { slug: group.slug, parentId: group.parent?.id ?? null })
    }

    repos.value = data.getAllRepositories
      .map((repo) => {
        const groupSegments = buildGroupPath(groupMap, repo.group?.id ?? null)
        const pathSegments = [...groupSegments, repo.slug]
        return {
          id: repo.id,
          path: pathSegments.join('/'),
          isRemote: repo.isRemote,
        }
      })
      .sort((a, b) => a.path.localeCompare(b.path))
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Unable to load repositories'
  } finally {
    loading.value = false
  }
}

async function createRepository(data: { slug: string; groupId?: string }) {
  try {
    const mutation = /* GraphQL */ `
      mutation CreateRepository($input: CreateRepositoryInput!) {
        createRepository(input: $input) {
          id
          slug
        }
      }
    `

    await graphqlRequest({
      query: mutation,
      variables: {
        input: {
          slug: data.slug,
          group: data.groupId || null
        }
      }
    })

    // Reload data to show the new repository
    await loadData()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to create repository'
  }
}

async function linkRepository(data: { url: string }) {
  try {
    const mutation = /* GraphQL */ `
      mutation LinkRemoteRepository($url: String!) {
        linkRemoteRepository(url: $url) {
          id
          slug
        }
      }
    `

    await graphqlRequest({
      query: mutation,
      variables: {
        url: data.url
      }
    })

    // Reload data to show the new repository
    await loadData()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to link repository'
  }
}

onMounted(async () => {
  await loadData()
})
</script>

<template>
  <div class="min-h-screen flex flex-col">
    <!-- Header -->
    <header class="sticky top-0 z-30 border-b bg-background/80 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="size-8 rounded bg-primary/10 text-primary grid place-items-center font-black">
            F
          </div>
          <span class="font-semibold">Forge</span>
        </div>
        <div class="hidden md:flex items-center gap-2 flex-1 max-w-xl mx-6">
          <div class="relative w-full">
            <UiInput type="search" placeholder="Search or jump to..." />
            <kbd class="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground border rounded px-1.5 py-0.5">/
            </kbd>
          </div>
        </div>
        <div class="w-12" />
      </div>
    </header>
    <!-- Repositories -->
    <section id="repos" class="">
      <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-16">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-2xl font-semibold">Repositories</h2>
          <div class="flex gap-2">
            <Button @click="showCreateDialog = true">
              Create Repository
            </Button>
            <Button variant="outline" @click="showLinkDialog = true">
              Link Repository
            </Button>
          </div>
        </div>
        <div class="grid gap-3">
          <div v-if="loading" class="rounded-lg border p-4 text-sm text-muted-foreground bg-muted/40">
            Loading repositories…
          </div>
          <div v-else-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
            {{ error }}
          </div>
          <div v-else-if="!hasRepos" class="rounded-lg border p-4 text-sm text-muted-foreground bg-muted/40">
            No repositories yet.
          </div>
          <a
            v-else
            v-for="r in repos"
            :key="r.id"
            :href="`/${r.path}`"
            class="rounded-lg border p-4 hover:bg-accent/40 transition-colors"
          >
            <div class="flex items-start justify-between gap-4">
              <div>
                <div class="font-medium">
                  <span v-if="r.path.includes('/')" class="text-muted-foreground">{{ r.path.split('/').slice(0,-1).join('/') }}/</span>
                  <span class="font-semibold">{{ r.path.split('/').slice(-1)[0] }}</span>
                </div>
              </div>
              <span
                v-if="r.isRemote"
                class="inline-flex items-center rounded border px-2 py-0.5 text-[10px] uppercase tracking-wide text-amber-700 border-amber-200 bg-amber-100/70"
              >Remote</span>
            </div>
          </a>
        </div>
      </div>
    </section>

    <!-- Dialogs -->
    <CreateRepositoryDialog 
      v-model:open="showCreateDialog" 
      :groups="groups"
      @create="createRepository"
    />
    <LinkRepositoryDialog 
      v-model:open="showLinkDialog" 
      @link="linkRepository"
    />

    <!-- Footer -->
    <footer class="border-t mt-auto">
      <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 text-sm text-muted-foreground flex flex-col sm:flex-row items-center justify-between gap-4">
        <div class="flex items-center gap-2">
          <div class="size-6 rounded bg-primary/10 text-primary grid place-items-center font-black">F</div>
          <span>Forge</span>
          <span>•</span>
          <span>© {{ new Date().getFullYear() }}</span>
        </div>
        <nav class="flex items-center gap-4">
          <a href="#" class="hover:text-foreground">Docs</a>
          <a href="#" class="hover:text-foreground">Status</a>
          <a href="#" class="hover:text-foreground">Terms</a>
          <a href="#" class="hover:text-foreground">Privacy</a>
        </nav>
      </div>
    </footer>
  </div>
</template>
