<script setup lang="ts">
import { computed, reactive } from 'vue'
import UiModal from './ui/modal.vue'
import UiButton from './ui/button.vue'
import UiInput from './ui/input.vue'

interface GroupOption {
  id: string
  slug: string
}

const props = withDefaults(
  defineProps<{
    open: boolean
    groups?: GroupOption[]
  }>(),
  {
    groups: () => [],
  },
)

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'create', value: { slug: string; groupId?: string }): void
}>()

const form = reactive({
  slug: '',
  groupId: '',
})

const isValid = computed(() => form.slug.length > 0 && /^[a-z0-9-]+$/.test(form.slug))

function reset() {
  form.slug = ''
  form.groupId = ''
}

function close() {
  emit('update:open', false)
  reset()
}

function submit() {
  if (!isValid.value) return
  emit('create', {
    slug: form.slug,
    groupId: form.groupId || undefined,
  })
  close()
}
</script>

<template>
  <UiModal :open="open" title="Create Repository" @update:open="(value) => value ? emit('update:open', value) : close()">
    <form class="space-y-4" @submit.prevent="submit">
      <div class="space-y-2">
        <label for="repo-name" class="text-sm font-medium text-foreground">
          Repository Name
          <span class="text-destructive">*</span>
        </label>
        <UiInput id="repo-name" v-model="form.slug" placeholder="my-awesome-repo" required />
        <p class="text-xs text-muted-foreground">Use lowercase letters, numbers, and hyphens only.</p>
      </div>

      <div v-if="groups.length" class="space-y-2">
        <label for="repo-group" class="text-sm font-medium text-foreground">Group (optional)</label>
        <select
          id="repo-group"
          v-model="form.groupId"
          class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        >
          <option value="">No group</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.slug }}</option>
        </select>
      </div>

      <div class="flex flex-col-reverse gap-2 pt-2 sm:flex-row sm:justify-end">
        <UiButton variant="outline" type="button" @click="close">Cancel</UiButton>
        <UiButton type="submit" :disabled="!isValid">Create Repository</UiButton>
      </div>
    </form>
  </UiModal>
</template>
