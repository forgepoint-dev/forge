<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { applyTheme, getBrand, getMode, setBrand, setMode, startSystemThemeWatcher, type ThemeBrand, type ThemeMode } from '../lib/theme'

const modes: { value: ThemeMode; label: string }[] = [
  { value: 'auto', label: 'Auto' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
]

const brands: { value: ThemeBrand; label: string }[] = [
  { value: 'rosepine', label: 'Rosé Pine' },
  { value: 'catppuccin', label: 'Catppuccin' },
]

const mode = ref<ThemeMode>(getMode())
const brand = ref<ThemeBrand>(getBrand())

onMounted(() => {
  applyTheme()
  startSystemThemeWatcher()
})

watch(mode, (m) => setMode(m))
watch(brand, (b) => setBrand(b))
</script>

<template>
  <div class="flex items-center gap-2">
    <label class="sr-only" for="theme-mode">Theme mode</label>
    <select id="theme-mode" v-model="mode" class="h-9 rounded-md border border-input bg-background px-2 text-sm">
      <option v-for="m in modes" :key="m.value" :value="m.value">{{ m.label }}</option>
    </select>

    <label class="sr-only" for="theme-brand">Theme palette</label>
    <select id="theme-brand" v-model="brand" class="h-9 rounded-md border border-input bg-background px-2 text-sm">
      <option v-for="b in brands" :key="b.value" :value="b.value">{{ b.label }}</option>
    </select>
  </div>
</template>

