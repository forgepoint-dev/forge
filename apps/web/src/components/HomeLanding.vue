<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiInput from './ui/input.vue'
import UiButton from './ui/button.vue'
import ThemeSwitcher from './ThemeSwitcher.vue'
import CreateRepositoryModal from './CreateRepositoryModal.vue'
import LinkRepositoryModal from './LinkRepositoryModal.vue'
import { graphqlRequest } from '../lib/graphql'

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
const authenticated = ref(false)

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

async function loadAuth() {
  try {
    const res = await fetch('/api/auth/me', { credentials: 'include' })
    if (res.ok) {
      const data = await res.json().catch(() => ({ authenticated: false }))
      authenticated.value = Boolean(data?.authenticated)
    }
  } catch {
    authenticated.value = false
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
  await Promise.all([loadAuth(), loadData()])
})
const authLoginUrl = (() => {
  const env = import.meta.env as Record<string, string | undefined>
  const override = env.PUBLIC_FORGE_AUTH_LOGIN_URL
  if (override) {
    return override
  }

  const graphqlEndpoint = env.PUBLIC_FORGE_GRAPHQL_URL ?? 'http://localhost:8000/graphql'
  const base = graphqlEndpoint.replace(/\/graphql$/, '')
  const sanitizedBase = base.endsWith('/') ? base.slice(0, -1) : base
  return `${sanitizedBase}/auth/login`
})()

</script>

<template>
  <div class="min-h-screen flex flex-col">
    <!-- Header moved to server island in layout -->
    <!-- Repositories -->
    <section id="repos" class="">
      <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-16">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-2xl font-semibold">Repositories</h2>
          <div class="flex gap-2">
            <template v-if="authenticated">
              <UiButton @click="showCreateDialog = true">
                Create Repository
              </UiButton>
              <UiButton variant="outline" @click="showLinkDialog = true">
                Link Repository
              </UiButton>
            </template>
            <template v-else>
              <a :href="`${authLoginUrl}?return_to=${encodeURIComponent(location.href)}`" class="inline-flex items-center justify-center gap-2 h-9 px-4 rounded-md text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90">
                Register / Login
              </a>
            </template>
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
    <CreateRepositoryModal
      v-model:open="showCreateDialog"
      :groups="groups"
      @create="createRepository"
    />
    <LinkRepositoryModal
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
