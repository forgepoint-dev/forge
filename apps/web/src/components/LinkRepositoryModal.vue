<script setup lang="ts">
import { reactive } from 'vue'
import UiModal from './ui/modal.vue'
import UiButton from './ui/button.vue'
import UiInput from './ui/input.vue'

const props = defineProps<{ open: boolean }>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'link', value: { url: string }): void
}>()

const form = reactive({
  url: '',
})

function reset() {
  form.url = ''
}

function close() {
  emit('update:open', false)
  reset()
}

function submit() {
  if (!form.url.trim()) return
  emit('link', { url: form.url.trim() })
  close()
}
</script>

<template>
  <UiModal :open="open" title="Link Remote Repository" @update:open="(value) => value ? emit('update:open', value) : close()">
    <form class="space-y-4" @submit.prevent="submit">
      <div class="space-y-2">
        <label for="remote-url" class="text-sm font-medium text-foreground">
          Remote URL
          <span class="text-destructive">*</span>
        </label>
        <UiInput id="remote-url" v-model="form.url" type="url" placeholder="https://git.example.com/group/repo.git" required />
        <p class="text-xs text-muted-foreground">
          Provide an HTTPS URL to the repository you want to mirror. Forge will perform a one-time fetch.
        </p>
      </div>

      <div class="flex flex-col-reverse gap-2 pt-2 sm:flex-row sm:justify-end">
        <UiButton variant="outline" type="button" @click="close">Cancel</UiButton>
        <UiButton type="submit" :disabled="!form.url.trim()">Link Repository</UiButton>
      </div>
    </form>
  </UiModal>
</template>
