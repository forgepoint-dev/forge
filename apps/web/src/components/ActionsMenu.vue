<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import UiButton from './ui/button.vue'
import type { ActionContext, ActionScope, RepositoryContext } from '../lib/slots'
import { actionSlots } from 'virtual:forge/slots/actions'

const props = withDefaults(
  defineProps<{
    scope: ActionScope
    repository?: RepositoryContext | null
    label?: string
  }>(),
  {
    repository: null,
    label: 'Actions',
  },
)

const menuRef = ref<HTMLElement | null>(null)
const open = ref(false)
const authenticated = ref(false)

const filteredActions = computed(() =>
  actionSlots
    .filter((slot) => slot.scope === props.scope)
    .map((slot) => ({
      ...slot,
      order: slot.order ?? 0,
    }))
    .sort((a, b) => a.order - b.order),
)

const hasActions = computed(() => filteredActions.value.length > 0)

function buildContext(): ActionContext {
  return {
    scope: props.scope,
    repository: props.repository ?? undefined,
    navigate: (path: string) => {
      if (typeof window !== 'undefined' && path) {
        window.location.href = path
      }
    },
  }
}

function interpolateHref(template: string | undefined): string | null {
  if (!template) return null
  const context = buildContext()
  const replacements: Record<string, string | undefined> = {
    'repository.id': context.repository?.id,
    'repository.slug': context.repository?.slug,
    'repository.fullPath': context.repository?.fullPath,
  }

  return template.replace(/\{([^}]+)\}/g, (_, key: string) => {
    const value = replacements[key.trim()]
    return value ?? ''
  })
}

async function runAction(actionIndex: number) {
  const action = filteredActions.value[actionIndex]
  if (!action) return

  const context = buildContext()

  try {
    if (action.kind === 'handler' && action.handler) {
      await action.handler(context)
    } else if (action.href) {
      const target = interpolateHref(action.href)
      if (target) {
        context.navigate(target)
      }
    } else {
      console.warn('[Forge Actions] Action has no handler or href:', action.id)
    }
  } catch (err) {
    console.error(`[Forge Actions] Action \`${action.id}\` failed`, err)
  } finally {
    open.value = false
  }
}

function toggleMenu() {
  if (!hasActions.value) return
  open.value = !open.value
}

function onDocumentClick(event: MouseEvent) {
  const target = event.target as Node | null
  if (!menuRef.value || !target) return
  if (!menuRef.value.contains(target)) {
    open.value = false
  }
}

onMounted(async () => {
  try {
    const res = await fetch('/api/auth/me', { credentials: 'include' })
    if (res.ok) {
      const data = await res.json().catch(() => ({ authenticated: false }))
      authenticated.value = Boolean(data?.authenticated)
    }
  } catch {
    authenticated.value = false
  }
  if (typeof document !== 'undefined') {
    document.addEventListener('click', onDocumentClick)
  }
})

onBeforeUnmount(() => {
  if (typeof document !== 'undefined') {
    document.removeEventListener('click', onDocumentClick)
  }
})
</script>

<template>
  <div v-if="hasActions && authenticated" ref="menuRef" class="relative">
    <UiButton variant="outline" @click.stop="toggleMenu">
      {{ label }}
      <svg
        class="ml-1 size-4"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 20 20"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      >
        <path stroke-linecap="round" stroke-linejoin="round" d="m6 8 4 4 4-4" />
      </svg>
    </UiButton>

    <transition name="fade">
      <div
        v-if="open"
        class="absolute right-0 z-50 mt-2 w-48 overflow-hidden rounded-md border border-border bg-popover shadow-lg"
      >
        <ul class="py-1 text-sm text-foreground">
          <li
            v-for="(action, index) in filteredActions"
            :key="action.id"
          >
            <button
              class="flex w-full items-center justify-between px-3 py-2 text-left hover:bg-accent hover:text-accent-foreground"
              type="button"
              @click.stop="runAction(index)"
            >
              <span>{{ action.label }}</span>
            </button>
          </li>
        </ul>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 120ms ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
