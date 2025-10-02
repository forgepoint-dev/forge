export type ThemeMode = 'light' | 'dark' | 'auto'
export type ThemeBrand = 'rosepine' | 'catppuccin'

const STORAGE_KEYS = {
  mode: 'fp-theme-mode',
  brand: 'fp-theme-brand',
} as const

function getStored<T extends string>(key: string): T | null {
  try {
    return (localStorage.getItem(key) as T | null)
  } catch {
    return null
  }
}

export function getMode(): ThemeMode {
  return (getStored<ThemeMode>(STORAGE_KEYS.mode) ?? 'auto')
}

export function getBrand(): ThemeBrand {
  return (getStored<ThemeBrand>(STORAGE_KEYS.brand) ?? 'rosepine')
}

export function setMode(mode: ThemeMode) {
  try { localStorage.setItem(STORAGE_KEYS.mode, mode) } catch {}
  applyTheme()
}

export function setBrand(brand: ThemeBrand) {
  try { localStorage.setItem(STORAGE_KEYS.brand, brand) } catch {}
  applyTheme()
}

let media: MediaQueryList | null = null

export function applyTheme() {
  const root = document.documentElement
  const mode = getMode()
  const brand = getBrand()

  root.setAttribute('data-theme', brand)

  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  const isDark = mode === 'dark' || (mode === 'auto' && prefersDark)
  root.classList.toggle('dark', isDark)
}

export function startSystemThemeWatcher() {
  if (media) return
  media = window.matchMedia('(prefers-color-scheme: dark)')
  const listener = () => {
    if (getMode() === 'auto') applyTheme()
  }
  try {
    media.addEventListener('change', listener)
  } catch {
    // Safari
    media!.addListener(listener)
  }
}

export function initThemeEarly() {
  // No-op placeholder for SSR guards when needed
}

