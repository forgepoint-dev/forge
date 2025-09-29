<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ modelValue?: string | number; type?: string }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: string | number): void }>()

const value = computed({
  get: () => props.modelValue as any,
  set: (v) => emit('update:modelValue', v),
})
</script>

<template>
  <input
    v-model="value"
    :type="props.type ?? 'text'"
    class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
  />
</template>

