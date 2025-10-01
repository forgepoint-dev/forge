<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

type ModalSize = 'sm' | 'md' | 'lg'

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    size?: ModalSize
    closeOnBackdrop?: boolean
  }>(),
  {
    size: 'md',
    closeOnBackdrop: true,
  },
)

const emit = defineEmits<{ (e: 'update:open', value: boolean): void }>()

const isMounted = ref(false)

onMounted(() => {
  isMounted.value = true
})

const sizeClass = computed(() => {
  switch (props.size) {
    case 'sm':
      return 'max-w-md'
    case 'lg':
      return 'max-w-3xl'
    case 'md':
    default:
      return 'max-w-xl'
  }
})

function close() {
  emit('update:open', false)
}

function onBackdropClick(event: MouseEvent) {
  if (!props.closeOnBackdrop) return
  if (event.target === event.currentTarget) {
    close()
  }
}
</script>

<template>
  <Teleport v-if="isMounted" to="body">
    <transition name="modal-fade">
      <div
        v-if="open"
        class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm px-4 py-8"
        @click="onBackdropClick"
      >
        <div
          class="relative w-full rounded-lg border border-border bg-card shadow-2xl"
          :class="sizeClass"
          role="dialog"
          aria-modal="true"
          @click.stop
        >
          <header v-if="title || $slots.header" class="flex items-start justify-between gap-4 border-b border-border px-6 py-4">
            <div class="space-y-1">
              <h2 v-if="title" class="text-lg font-semibold text-foreground">{{ title }}</h2>
              <slot name="header" />
            </div>
            <button
              type="button"
              class="inline-flex size-8 items-center justify-center rounded-md text-muted-foreground transition hover:text-foreground"
              @click="close"
            >
              <span class="sr-only">Close dialog</span>
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-4">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
              </svg>
            </button>
          </header>

          <div class="px-6 py-5 space-y-4">
            <slot />
          </div>

          <footer v-if="$slots.footer" class="flex flex-col-reverse gap-2 border-t border-border px-6 py-4 sm:flex-row sm:justify-end">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity 150ms ease;
}

.modal-fade-enter-from,
.modal-fade-leave-to {
  opacity: 0;
}
</style>
