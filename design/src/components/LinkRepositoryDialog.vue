<script setup lang="ts">
import { ref, computed } from 'vue'
import Dialog from './ui/Dialog.vue'
import Button from './ui/Button.vue'
import Input from './ui/Input.vue'
import Label from './ui/Label.vue'

interface Props {
  open?: boolean
}

interface Emits {
  (e: 'update:open', open: boolean): void
  (e: 'link', data: { url: string }): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

const url = ref('')
const isSubmitting = ref(false)

const isValid = computed(() => {
  try {
    const parsedUrl = new URL(url.value)
    return parsedUrl.protocol === 'https:' || parsedUrl.protocol === 'http:'
  } catch {
    return false
  }
})

const onClose = () => {
  emit('update:open', false)
  resetForm()
}

const resetForm = () => {
  url.value = ''
  isSubmitting.value = false
}

const onSubmit = async (e: Event) => {
  e.preventDefault()
  if (!isValid.value || isSubmitting.value) return

  isSubmitting.value = true
  try {
    emit('link', { url: url.value })
    onClose()
  } finally {
    isSubmitting.value = false
  }
}
</script>

<template>
  <Dialog 
    :open="open" 
    title="Link Remote Repository"
    @update:open="(open) => emit('update:open', open)"
  >
    <form @submit="onSubmit" class="space-y-4">
      <div class="space-y-2">
        <Label for="repo-url" required>Repository URL</Label>
        <Input
          id="repo-url"
          v-model="url"
          type="url"
          placeholder="https://github.com/username/repository.git"
          required
        />
        <p class="text-xs text-muted-foreground">
          Enter the HTTP(S) URL of a Git repository you want to link.
        </p>
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
        {{ isSubmitting ? 'Linking...' : 'Link Repository' }}
      </Button>
    </template>
  </Dialog>
</template>