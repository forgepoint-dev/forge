<script setup lang="ts">
type Variant = 'default' | 'secondary' | 'destructive' | 'outline' | 'ghost' | 'link'
type Size = 'sm' | 'md' | 'lg' | 'icon'

const props = withDefaults(defineProps<{ variant?: Variant; size?: Size; as?: string }>(), {
  variant: 'default',
  size: 'md',
  as: 'button',
})

const base =
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 ring-offset-background'

const variantClasses: Record<Variant, string> = {
  default: 'bg-primary text-primary-foreground hover:bg-primary/90',
  secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
  destructive: 'bg-destructive text-destructive-foreground hover:bg-destructive/90',
  outline: 'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
  ghost: 'hover:bg-accent hover:text-accent-foreground',
  link: 'text-primary underline-offset-4 hover:underline',
}

const sizeClasses: Record<Size, string> = {
  sm: 'h-8 px-3',
  md: 'h-9 px-4',
  lg: 'h-10 px-8',
  icon: 'h-9 w-9',
}
</script>

<template>
  <component
    :is="props.as"
    type="button"
    :class="[base, variantClasses[props.variant], sizeClasses[props.size]]"
  >
    <slot />
  </component>
  
</template>

