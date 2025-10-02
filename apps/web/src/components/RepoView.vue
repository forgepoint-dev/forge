<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ExpressiveCodeEngine } from '@expressive-code/core'
import { toHtml } from '@expressive-code/core/hast'
import { loadShikiTheme, pluginShiki } from '@expressive-code/plugin-shiki'
import { pluginLineNumbers } from '@expressive-code/plugin-line-numbers'
import UiButton from './ui/button.vue'
import UiInput from './ui/input.vue'
import ExtensionTabs from './ExtensionTabs.vue'
import ActionsMenu from './ActionsMenu.vue'
import { graphqlRequest } from '../lib/graphql'
import type { RepositoryContext } from '../lib/slots'

let expressiveEnginePromise: Promise<ExpressiveCodeEngine> | null = null
let expressiveGlobalsPromise: Promise<void> | null = null
const expressiveStyleRegistry = new Set<string>()
const expressiveScriptRegistry = new Set<string>()

async function getExpressiveEngine() {
  if (!expressiveEnginePromise) {
    expressiveEnginePromise = (async () => {
      const [lightTheme, darkTheme] = await Promise.all([
        loadShikiTheme('rose-pine-dawn'),
        loadShikiTheme('rose-pine-moon'),
      ])

      return new ExpressiveCodeEngine({
        themes: [lightTheme, darkTheme],
        useDarkModeMediaQuery: false,
        themeCssSelector: (theme) =>
          theme.type === 'dark' ? '.dark' : ':root:not(.dark)',
        plugins: [
          pluginShiki({
            engine: 'javascript',
            langAlias: {
              md: 'md',
              markdown: 'md',
              yml: 'yaml',
            },
          }),
          pluginLineNumbers(),
        ],
      })
    })()
  }

  return expressiveEnginePromise
}

async function ensureExpressiveGlobals(engine: ExpressiveCodeEngine) {
  if (typeof window === 'undefined') {
    return
  }

  if (!expressiveGlobalsPromise) {
    expressiveGlobalsPromise = (async () => {
      const [baseStyles, themeStyles, jsModules] = await Promise.all([
        engine.getBaseStyles(),
        engine.getThemeStyles(),
        engine.getJsModules(),
      ])

      injectExpressiveStyle(baseStyles, 'base')
      injectExpressiveStyle(themeStyles, 'themes')

      for (const moduleCode of jsModules) {
        injectExpressiveModule(moduleCode)
      }
    })()
  }

  await expressiveGlobalsPromise
}

function injectExpressiveStyle(css: string, marker: string) {
  if (!css || typeof window === 'undefined' || expressiveStyleRegistry.has(css)) {
    return
  }

  const styleEl = document.createElement('style')
  styleEl.setAttribute('data-expressive-code', marker)
  styleEl.textContent = css
  document.head.append(styleEl)
  expressiveStyleRegistry.add(css)
}

function injectExpressiveModule(code: string) {
  if (!code || typeof window === 'undefined' || expressiveScriptRegistry.has(code)) {
    return
  }

  const script = document.createElement('script')
  script.type = 'module'
  script.setAttribute('data-expressive-code', 'module')
  script.textContent = code
  document.head.append(script)
  expressiveScriptRegistry.add(code)
}

async function renderExpressiveHtml(code: string, language: string | null) {
  const engine = await getExpressiveEngine()
  await ensureExpressiveGlobals(engine)

  let result
  try {
    result = await engine.render({ code, language: language ?? undefined })
  } catch (err) {
    if (language) {
      result = await engine.render({ code })
    } else {
      throw err
    }
  }

  if (typeof window !== 'undefined') {
    for (const style of result.styles) {
      injectExpressiveStyle(style, 'block')
    }
  }

  return toHtml(result.renderedGroupAst)
}

const props = defineProps<{ fullPath: string }>()

type RepositoryDetails = {
  id: string
  slug: string
  group: { id: string; slug: string } | null
  isRemote: boolean
  remoteUrl?: string | null
  readmeHtml?: string | null
}

type GroupDetails = {
  id: string
  slug: string
  parent: { id: string; slug: string } | null
  repositories: { id: string; slug: string; isRemote: boolean; remoteUrl?: string | null }[]
}

type RepositoryEntry = {
  name: string
  path: string
  type: 'FILE' | 'DIRECTORY'
  size: number | null
}

type RepositoryEntriesResponse = {
  treePath: string
  entries: RepositoryEntry[]
}

type RepositoryFileResponse = {
  path: string
  name: string
  size: number
  isBinary: boolean
  text: string | null
  truncated: boolean
}

const PREVIEW_LIMIT_BYTES = 128 * 1024

const segments = computed(() => props.fullPath.split('/').filter(Boolean))
const repoName = computed(() => segments.value[segments.value.length - 1] || '')
const groups = computed(() => segments.value.slice(0, -1))
const badge = computed(() => (groups.value[0]?.[0] || repoName.value[0] || 'F').toUpperCase())

const repository = ref<RepositoryDetails | null>(null)
const groupDetails = ref<GroupDetails | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)

const treePath = ref('')
const resolvedTreePath = ref('')
const entries = ref<RepositoryEntry[]>([])
const entriesLoading = ref(false)
const entriesError = ref<string | null>(null)
const selectedFilePath = ref<string | null>(null)
const fileContent = ref<RepositoryFileResponse | null>(null)
const fileLoading = ref(false)
const fileError = ref<string | null>(null)
const filePreviewHtml = ref<string | null>(null)

const isRemote = computed(() => repository.value?.isRemote ?? false)
const remoteUrl = computed(() => repository.value?.remoteUrl ?? null)

const repositoryContext = computed<RepositoryContext | null>(() => {
  if (!repository.value) return null
  return {
    version: 1,
    id: repository.value.id,
    slug: repository.value.slug,
    fullPath: props.fullPath,
    isRemote: repository.value.isRemote,
    remoteUrl: repository.value.remoteUrl ?? null,
  }
})

const siblingRepositories = computed(() => {
  if (!groupDetails.value) return []
  return groupDetails.value.repositories.filter((repo) => repo.id !== repository.value?.id)
})

const breadcrumbItems = computed(() => {
  const crumbs: { label: string; path: string }[] = []
  const rootLabel = repoName.value || 'Repository'
  crumbs.push({ label: rootLabel, path: '' })

  let current = ''
  const segments = resolvedTreePath.value.split('/').filter(Boolean)
  for (const segment of segments) {
    current = current ? `${current}/${segment}` : segment
    crumbs.push({ label: segment, path: current })
  }

  return crumbs
})

const hasEntries = computed(() => entries.value.length > 0)
const selectedFileName = computed(() => {
  if (fileContent.value) return fileContent.value.name
  if (!selectedFilePath.value) return ''
  const segments = selectedFilePath.value.split('/').filter(Boolean)
  return segments[segments.length - 1] || ''
})

const isBinaryFile = computed(() => fileContent.value?.isBinary ?? false)

const isEmptyTextFile = computed(() => {
  if (!fileContent.value || fileContent.value.isBinary) return false
  return (fileContent.value.text ?? '').length === 0
})

const fileSizeFormatted = computed(() => {
  if (!fileContent.value) return '—'
  return formatSize(fileContent.value.size)
})

function guessLanguage(filename: string): string | null {
  const ext = filename.split('.').pop()?.toLowerCase()
  if (!ext) return null

  const lookup: Record<string, string> = {
    ts: 'ts',
    tsx: 'tsx',
    js: 'js',
    jsx: 'jsx',
    json: 'json',
    rs: 'rust',
    ron: 'ron',
    toml: 'toml',
    md: 'md',
    markdown: 'md',
    sh: 'bash',
    bash: 'bash',
    yml: 'yaml',
    yaml: 'yaml',
    css: 'css',
    scss: 'scss',
    html: 'html',
    vue: 'vue',
    astro: 'astro',
    graphql: 'graphql',
    gql: 'graphql',
    sql: 'sql',
    py: 'python',
    go: 'go',
  }

  return lookup[ext] ?? null
}

async function loadData(path: string) {
  loading.value = true
  error.value = null
  repository.value = null
  groupDetails.value = null
  entries.value = []
  entriesError.value = null
  entriesLoading.value = false
  treePath.value = ''
  resolvedTreePath.value = ''
  selectedFilePath.value = null
  fileContent.value = null
  fileError.value = null
  fileLoading.value = false
  filePreviewHtml.value = null
  branches.value = []
  branchesError.value = null
  branch.value = ''
  branchInitialized = false
  activeBranchReference = null

  try {
    const hasBranch = !!branch.value
    const queryWithBranch = /* GraphQL */ `
      query RepositoryByPath($path: String!, $branch: String) {
        getRepository(path: $path) {
          id
          slug
          isRemote
          remoteUrl
          readmeHtml(branch: $branch)
          group { id slug }
        }
      }
    `
    const queryWithoutBranch = /* GraphQL */ `
      query RepositoryByPathNoBranch($path: String!) {
        getRepository(path: $path) {
          id
          slug
          isRemote
          remoteUrl
          readmeHtml
          group { id slug }
        }
      }
    `

    const repoResponse = await graphqlRequest<{ getRepository: RepositoryDetails | null }>({
      query: hasBranch ? queryWithBranch : queryWithoutBranch,
      variables: hasBranch ? { path, branch: branch.value } : { path },
    })

    if (!repoResponse.getRepository) {
      error.value = 'Repository not found.'
      return
    }

    repository.value = repoResponse.getRepository

    await loadBranches(path)
    await loadRepositoryEntries(path, treePath.value)

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

async function loadBranches(path: string) {
  branchesLoading.value = true
  branchesError.value = null

  try {
    const response = await graphqlRequest<{ listRepositoryBranches: BranchOption[] | null }>({
      query: /* GraphQL */ `
        query RepositoryBranches($path: String!) {
          listRepositoryBranches(path: $path) {
            name
            reference
            isDefault
          }
        }
      `,
      variables: { path },
    })

    const payload = response.listRepositoryBranches ?? []
    branches.value = payload

    const defaultBranch =
      payload.find((item) => item.isDefault) ?? payload[0] ?? null

    branchInitialized = true
    if (defaultBranch) {
      branch.value = defaultBranch.reference
    } else {
      branch.value = ''
    }
  } catch (err) {
    branches.value = []
    branch.value = ''
    branchInitialized = true
    branchesError.value = err instanceof Error ? err.message : 'Failed to load branches'
  } finally {
    branchesLoading.value = false
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

type BranchOption = {
  name: string
  reference: string
  isDefault: boolean
}

const branches = ref<BranchOption[]>([])
const branch = ref<string>('')
const branchesLoading = ref(false)
const branchesError = ref<string | null>(null)
let branchInitialized = false
let activeBranchReference: string | null = null

let entriesRequestId = 0

async function loadRepositoryEntries(path: string, tree: string) {
  const requestId = ++entriesRequestId
  entriesLoading.value = true
  entriesError.value = null

  try {
    const variables: { path: string; treePath: string; branch?: string | null } = {
      path,
      treePath: tree,
    }

    if (branch.value) {
      variables.branch = branch.value
    }

    const response = await graphqlRequest<{ browseRepository: RepositoryEntriesResponse | null }>({
      query: /* GraphQL */ `
        query BrowseRepository($path: String!, $treePath: String, $branch: String) {
          browseRepository(path: $path, treePath: $treePath, branch: $branch) {
            treePath
            entries {
              name
              path
              type
              size
            }
          }
        }
      `,
      variables,
    })

    if (requestId !== entriesRequestId) {
      return
    }

    const payload = response.browseRepository
    if (!payload) {
      entries.value = []
      entriesError.value = 'Repository contents are unavailable.'
      resolvedTreePath.value = tree
      activeBranchReference = branch.value || null
      return
    }

    entries.value = payload.entries
    activeBranchReference = branch.value || null
    if (selectedFilePath.value) {
      const stillPresent = payload.entries.some((entry) => entry.path === selectedFilePath.value)
      if (!stillPresent) {
        selectedFilePath.value = null
        fileContent.value = null
        fileError.value = null
        fileLoading.value = false
        filePreviewHtml.value = null
      }
    }
    resolvedTreePath.value = payload.treePath
  } catch (err) {
    if (requestId !== entriesRequestId) {
      return
    }
    entries.value = []
    activeBranchReference = branch.value || null
    selectedFilePath.value = null
    fileContent.value = null
    fileError.value = null
    fileLoading.value = false
    filePreviewHtml.value = null
    entriesError.value = err instanceof Error ? err.message : 'Failed to load repository contents'
    resolvedTreePath.value = tree
  } finally {
    if (requestId === entriesRequestId) {
      entriesLoading.value = false
    }
  }
}


async function openFile(path: string) {
  if (!repository.value) return
  if (selectedFilePath.value === path && fileContent.value && !fileError.value) {
    return
  }

  selectedFilePath.value = path
  fileLoading.value = true
  fileError.value = null
  fileContent.value = null
  filePreviewHtml.value = null

  try {
    const response = await graphqlRequest<{ readRepositoryFile: RepositoryFileResponse | null }>({
      query: /* GraphQL */ `
        query ReadRepositoryFile($path: String!, $filePath: String!, $branch: String) {
          readRepositoryFile(path: $path, filePath: $filePath, branch: $branch) {
            path
            name
            size
            isBinary
            text
            truncated
          }
        }
      `,
      variables: {
        path: props.fullPath,
        filePath: path,
        branch: branch.value || null,
      },
    })

    const payload = response.readRepositoryFile
    if (!payload) {
      fileError.value = 'File preview is unavailable.'
      return
    }

    fileContent.value = payload

    if (!payload.isBinary && payload.text !== null) {
      try {
        filePreviewHtml.value = await renderExpressiveHtml(
          payload.text,
          guessLanguage(payload.name),
        )
      } catch (renderErr) {
        console.error(renderErr)
        filePreviewHtml.value = null
      }
    }
  } catch (err) {
    fileError.value = err instanceof Error ? err.message : 'Failed to load file'
  } finally {
    fileLoading.value = false
  }
}

watch(
  treePath,
  (next, prev) => {
    if (!repository.value || next === prev) {
      return
    }
    selectedFilePath.value = null
    fileContent.value = null
    fileError.value = null
    fileLoading.value = false
    filePreviewHtml.value = null
    loadRepositoryEntries(props.fullPath, next)
  },
)

watch(
  branch,
  async (next, prev) => {
    if (!branchInitialized || next === prev) {
      return
    }
    if (!repository.value) {
      return
    }

    const normalized = next || null
    if (normalized === activeBranchReference) {
      return
    }

    selectedFilePath.value = null
    fileContent.value = null
    fileError.value = null
    fileLoading.value = false
    filePreviewHtml.value = null

    // Refetch README for new branch (or default when cleared)
    try {
      const hasBranch = !!next
      const queryWithBranch = /* GraphQL */ `
        query RepositoryReadmeForBranch($path: String!, $branch: String) {
          getRepository(path: $path) { id readmeHtml(branch: $branch) }
        }
      `
      const queryWithoutBranch = /* GraphQL */ `
        query RepositoryReadmeForBranchNoBranch($path: String!) {
          getRepository(path: $path) { id readmeHtml }
        }
      `

      const repoResponse = await graphqlRequest<{ getRepository: RepositoryDetails | null }>({
        query: hasBranch ? queryWithBranch : queryWithoutBranch,
        variables: hasBranch ? { path: props.fullPath, branch: next } : { path: props.fullPath },
      })

      if (repoResponse.getRepository && repository.value) {
        repository.value.readmeHtml = repoResponse.getRepository.readmeHtml
      }
    } catch (err) {
      console.error('Failed to load README for branch:', err)
    }

    const resetPath = ''
    if (treePath.value !== resetPath) {
      treePath.value = resetPath
    } else {
      loadRepositoryEntries(props.fullPath, resetPath)
    }
  },
)

function navigateToTree(target: string) {
  if (treePath.value === target) return
  selectedFilePath.value = null
  fileContent.value = null
  fileError.value = null
  fileLoading.value = false
  filePreviewHtml.value = null
  treePath.value = target
}

function formatSize(size: number | null | undefined) {
  if (size === null || size === undefined) return '—'
  if (size < 1024) return `${size} B`

  const units = ['KB', 'MB', 'GB', 'TB']
  let value = size
  let unitIndex = 0

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }

  const formatted = value >= 100 || unitIndex === 0 ? Math.round(value).toString() : value.toFixed(1)
  return `${formatted} ${units[unitIndex]}`
}

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
            <ActionsMenu scope="repository" :repository="repositoryContext ?? undefined" />
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
          <div class="flex flex-col gap-1">
            <div class="flex items-center gap-2">
              <label class="text-xs text-muted-foreground">Branch</label>
              <select
                v-model="branch"
                :disabled="branchesLoading || branches.length === 0"
                class="h-9 min-w-[10rem] rounded-md border border-input bg-background px-2 text-sm disabled:opacity-60"
              >
                <option v-if="branchesLoading" value="">Loading…</option>
                <option v-else-if="branches.length === 0" value="">HEAD</option>
                <option
                  v-for="b in branches"
                  :key="b.reference"
                  :value="b.reference"
                >
                  {{ b.isDefault ? `${b.name} (default)` : b.name }}
                </option>
              </select>
            </div>
            <p v-if="branchesError" class="text-xs text-destructive">{{ branchesError }}</p>
          </div>
          <div class="flex items-center gap-2 w-full sm:w-80">
            <UiInput placeholder="Search this repo" />
            <UiButton variant="outline">Go</UiButton>
          </div>
        </div>

        <ExtensionTabs v-if="repositoryContext" :repository="repositoryContext">
          <template #readme>
            <section v-if="repository?.readmeHtml && resolvedTreePath === ''" class="rounded-lg border bg-card">
              <div class="border-b px-5 py-4">
                <h3 class="text-sm font-semibold text-muted-foreground flex items-center gap-2">
                  <span>README</span>
                </h3>
              </div>
              <div class="prose prose-sm dark:prose-invert max-w-none p-5">
                <div v-html="repository.readmeHtml"></div>
              </div>
            </section>
            <section v-else class="rounded-lg border bg-card p-5 text-sm text-muted-foreground">
              <span v-if="resolvedTreePath !== ''">README only shown at repository root.</span>
              <span v-else>No README available.</span>
            </section>
          </template>
          <template #files>
            <section class="rounded-lg border bg-card">
          <div class="flex flex-col gap-2 border-b px-5 py-4 sm:flex-row sm:items-center sm:justify-between">
            <h3 class="text-sm font-semibold text-muted-foreground">Repository Contents</h3>
            <nav class="flex flex-wrap items-center gap-1 text-xs text-muted-foreground sm:text-sm">
              <template v-for="(crumb, index) in breadcrumbItems" :key="`${crumb.path}:${index}`">
                <button
                  v-if="index !== breadcrumbItems.length - 1"
                  type="button"
                  class="rounded px-1.5 py-0.5 transition hover:text-foreground"
                  @click="navigateToTree(crumb.path)"
                >
                  {{ crumb.label }}
                </button>
                <span
                  v-else
                  class="rounded px-1.5 py-0.5 font-semibold text-foreground"
                >
                  {{ crumb.label }}
                </span>
                <span v-if="index < breadcrumbItems.length - 1" class="text-muted-foreground">/</span>
              </template>
            </nav>
          </div>
          <div class="space-y-3 p-5 text-sm">
            <div
              v-if="entriesLoading"
              class="rounded-md border border-dashed px-4 py-6 text-center text-muted-foreground"
            >
              Loading repository contents…
            </div>
            <div
              v-else-if="entriesError"
              class="rounded-md border border-destructive/50 bg-destructive/10 px-4 py-6 text-center text-destructive"
            >
              {{ entriesError }}
            </div>
            <div
              v-else-if="!hasEntries"
              class="rounded-md border border-dashed px-4 py-6 text-center text-muted-foreground"
            >
              This folder is empty.
            </div>
            <div v-else class="overflow-hidden rounded-md border">
              <table class="min-w-full divide-y text-sm">
                <thead class="bg-muted/40 text-xs uppercase tracking-wide text-muted-foreground">
                  <tr>
                    <th scope="col" class="px-4 py-2 text-left font-semibold">Name</th>
                    <th scope="col" class="px-4 py-2 text-left font-semibold">Type</th>
                    <th scope="col" class="px-4 py-2 text-right font-semibold">Size</th>
                  </tr>
                </thead>
                <tbody class="divide-y bg-background/60">
                  <tr
                    v-for="entry in entries"
                    :key="entry.path"
                    class="transition hover:bg-muted/50"
                    :class="{
                      'bg-muted/50': entry.type === 'DIRECTORY'
                        ? resolvedTreePath === entry.path
                        : selectedFilePath === entry.path,
                    }"
                  >
                    <td class="px-4 py-2">
                      <button
                        v-if="entry.type === 'DIRECTORY'"
                        type="button"
                        class="w-full font-medium text-left text-foreground hover:underline"
                        @click="navigateToTree(entry.path)"
                      >
                        {{ entry.name }}
                      </button>
                      <button
                        v-else
                        type="button"
                        class="w-full font-medium text-left text-foreground hover:underline"
                        @click="openFile(entry.path)"
                      >
                        {{ entry.name }}
                      </button>
                    </td>
                    <td class="px-4 py-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      {{ entry.type === 'DIRECTORY' ? 'Directory' : 'File' }}
                    </td>
                    <td class="px-4 py-2 text-right text-xs text-muted-foreground">
                      {{ entry.type === 'DIRECTORY' ? '—' : formatSize(entry.size) }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-if="selectedFilePath" class="overflow-hidden rounded-md border">
              <div class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/40 px-4 py-3">
                <div>
                  <p class="font-medium text-foreground">{{ selectedFileName || selectedFilePath }}</p>
                  <p class="text-xs text-muted-foreground">
                    {{ fileSizeFormatted }}
                    <span v-if="fileContent?.truncated">
                      · Showing first {{ Math.floor(PREVIEW_LIMIT_BYTES / 1024) }} KB
                    </span>
                  </p>
                </div>
              </div>
              <div class="max-h-[32rem] overflow-auto bg-background font-mono text-sm">
                <div
                  v-if="fileLoading"
                  class="px-4 py-6 text-center text-muted-foreground"
                >
                  Loading file…
                </div>
                <div
                  v-else-if="fileError"
                  class="m-4 rounded border border-destructive/30 bg-destructive/10 px-4 py-6 text-center text-destructive"
                >
                  {{ fileError }}
                </div>
                <div
                  v-else-if="isBinaryFile"
                  class="px-4 py-6 text-center text-muted-foreground"
                >
                  This file is binary and cannot be previewed.
                </div>
                <div
                  v-else-if="isEmptyTextFile"
                  class="px-4 py-6 text-center text-muted-foreground"
                >
                  This file is empty.
                </div>
                <div
                  v-else-if="fileContent && fileContent.text"
                  class="border-t border-border"
                >
                  <div
                    v-if="filePreviewHtml"
                    class="expressive-code-viewer"
                    v-html="filePreviewHtml"
                  ></div>
                  <pre v-else class="overflow-x-auto whitespace-pre px-4 py-3 text-xs">{{ fileContent.text }}</pre>
                  <p
                    v-if="fileContent.truncated"
                    class="px-4 pb-4 pt-2 text-xs text-muted-foreground"
                  >
                    Preview truncated. Showing first {{ Math.floor(PREVIEW_LIMIT_BYTES / 1024) }} KB.
                  </p>
                </div>
                <div
                  v-else
                  class="px-4 py-6 text-center text-muted-foreground"
                >
                  No preview available for this file.
                </div>
              </div>
            </div>
          </div>
        </section>
          </template>
        </ExtensionTabs>

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

<style scoped>
.expressive-code-viewer {
  padding: 0;
}

.expressive-code-viewer :deep(.expressive-code) {
  margin: 0;
  background-color: transparent;
}

.expressive-code-viewer :deep(.expressive-code pre) {
  font-size: 0.875rem;
}

.expressive-code-viewer :deep(.expressive-code .ec-line-numbers) {
  user-select: none;
}
</style>
