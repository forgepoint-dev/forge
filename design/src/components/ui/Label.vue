<script setup lang="ts">
interface Props {
  for?: string
  required?: boolean
}

defineProps<Props>()
</script>

<template>
  <label 
    :for="for"
    class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
  >
    <slot />
    <span v-if="required" class="text-destructive ml-1">*</span>
  </label>
</template>