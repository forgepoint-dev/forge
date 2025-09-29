<script setup lang="ts">
interface Props {
  modelValue?: string | number
  type?: string
  placeholder?: string
  disabled?: boolean
  required?: boolean
  class?: string
}

interface Emits {
  (e: 'update:modelValue', value: string | number): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

const onInput = (e: Event) => {
  const target = e.target as HTMLInputElement
  emit('update:modelValue', target.value)
}
</script>

<template>
  <input
    :value="modelValue"
    :type="type || 'text'"
    :placeholder="placeholder"
    :disabled="disabled"
    :required="required"
    :class="[
      'flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50',
      $props.class
    ]"
    @input="onInput"
  />
</template>