<script setup lang="ts">
import { onMounted, ref } from 'vue'

interface Props {
  open?: boolean
  title?: string
}

interface Emits {
  (e: 'update:open', open: boolean): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

const isMounted = ref(false)

onMounted(() => {
  isMounted.value = true
})

const onClose = () => {
  emit('update:open', false)
}

const onBackdropClick = (e: Event) => {
  if (e.target === e.currentTarget) {
    onClose()
  }
}
</script>

<template>
  <Teleport v-if="isMounted" to="body">
    <div 
      v-if="open" 
      class="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm"
      @click="onBackdropClick"
    >
      <div class="fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 border bg-background p-6 shadow-lg duration-200 sm:rounded-lg">
        <div class="flex flex-col space-y-1.5 text-center sm:text-left">
          <h2 v-if="title" class="text-lg font-semibold leading-none tracking-tight">
            {{ title }}
          </h2>
          <slot name="header" />
        </div>
        <slot />
        <div v-if="$slots.footer" class="flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>