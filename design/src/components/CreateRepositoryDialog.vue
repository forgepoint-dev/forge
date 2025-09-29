<script setup lang="ts">
import { ref, computed } from 'vue'
import Dialog from './ui/Dialog.vue'
import Button from './ui/Button.vue'
import Input from './ui/Input.vue'
import Label from './ui/Label.vue'

interface Props {
  open?: boolean
  groups?: Array<{ id: string; slug: string }>
}

interface Emits {
  (e: 'update:open', open: boolean): void
  (e: 'create', data: { slug: string; groupId?: string }): void
}

withDefaults(defineProps<Props>(), {
  groups: () => []
})
const emit = defineEmits<Emits>()

const slug = ref('')
const selectedGroupId = ref('')
const isSubmitting = ref(false)

const isValid = computed(() => {
  return slug.value.length > 0 && /^[a-z0-9-]+$/.test(slug.value)
})

const onClose = () => {
  emit('update:open', false)
  resetForm()
}

const resetForm = () => {
  slug.value = ''
  selectedGroupId.value = ''
  isSubmitting.value = false
}

const onSubmit = async (e: Event) => {
  e.preventDefault()
  if (!isValid.value || isSubmitting.value) return

  isSubmitting.value = true
  try {
    emit('create', {
      slug: slug.value,
      groupId: selectedGroupId.value || undefined
    })
    onClose()
  } finally {
    isSubmitting.value = false
  }
}
</script>

<template>
  <Dialog 
    :open="open" 
    title="Create Repository"
    @update:open="(open) => emit('update:open', open)"
  >
    <form @submit="onSubmit" class="space-y-4">
      <div class="space-y-2">
        <Label for="repo-slug" required>Repository Name</Label>
        <Input
          id="repo-slug"
          v-model="slug"
          placeholder="my-awesome-repo"
          required
        />
        <p class="text-xs text-muted-foreground">
          Use lowercase letters, numbers, and hyphens only.
        </p>
      </div>

      <div v-if="groups.length > 0" class="space-y-2">
        <Label for="group-select">Group (optional)</Label>
        <select
          id="group-select"
          v-model="selectedGroupId"
          class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <option value="">No group</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">
            {{ group.slug }}
          </option>
        </select>
      </div>
    </form>

    <template #footer>
      <Button variant="outline" @click="onClose">
        Cancel
      </Button>
      <Button 
        type="submit" 
        :disabled="!isValid || isSubmitting"
        @click="onSubmit"
      >
        {{ isSubmitting ? 'Creating...' : 'Create Repository' }}
      </Button>
    </template>
  </Dialog>
</template>