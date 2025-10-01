# ADR-0007: Design System (Storybook Components)

- Status: Accepted
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Context

Forgepoint needs a centralized component library to:
- Ensure visual consistency across the web application
- Provide reusable UI components (buttons, inputs, dialogs)
- Enable rapid UI development with pre-built primitives
- Document components with examples and usage guidelines
- Support isolated component development and testing
- Share components between core app and extensions

The design system should be lightweight, maintainable, and aligned with modern frontend practices.

## Decision

We implemented a **Vue 3 component library with Storybook** as the design system, providing reusable UI primitives and composite components.

### 1. Technology Stack

**Component Framework: Vue 3**
- Single-file components (SFC) with `<script setup>` syntax
- Composition API for better TypeScript integration
- Reactive props and events for component communication
- Scoped styles for encapsulation

**Documentation Platform: Storybook 9**
- Interactive component explorer
- Isolated component development
- Automatic documentation generation
- Accessibility testing with `@storybook/addon-a11y`
- Component testing with `@storybook/addon-vitest`

**Build Tool: Vite + TypeScript**
- Fast HMR for development
- TypeScript type checking with `vue-tsc`
- Optimized production builds

**Package Manager: Bun 1.1.30**
- Fast installation and builds
- Workspace support for monorepo

### 2. Component Architecture

#### UI Primitives (`src/components/ui/`)

Low-level, reusable building blocks:

- **Button.vue**: Primary action button with variants
  - Props: `label`, `variant` (primary/secondary), `size`
  - Events: `click`
  - States: Normal, hover, disabled

- **Input.vue**: Text input field
  - Props: `modelValue`, `placeholder`, `type`, `disabled`
  - Events: `update:modelValue`
  - v-model support for two-way binding

- **Label.vue**: Form label component
  - Props: `for`, `required`
  - Semantic HTML with proper accessibility

- **Dialog.vue**: Modal dialog container
  - Props: `open`, `title`, `description`
  - Events: `update:open`
  - Slots: Default slot for content, actions slot for buttons
  - Focus trap and keyboard navigation (Escape to close)

#### Composite Components (`src/components/`)

Higher-level components combining primitives:

- **CreateRepositoryDialog.vue**: Repository creation form
  - Uses Dialog, Input, Label, Button
  - Form validation and submission
  - GraphQL mutation integration

- **LinkRepositoryDialog.vue**: Remote repository linking form
  - Uses Dialog, Input, Label, Button
  - URL input with validation
  - GraphQL mutation integration

### 3. Project Structure

```
design/
├── src/
│   ├── components/
│   │   ├── ui/                    # UI primitives
│   │   │   ├── Button.vue
│   │   │   ├── Input.vue
│   │   │   ├── Label.vue
│   │   │   └── Dialog.vue
│   │   ├── CreateRepositoryDialog.vue
│   │   ├── LinkRepositoryDialog.vue
│   │   └── index.ts              # Component exports
│   ├── stories/                   # Storybook stories
│   │   ├── Button.stories.ts
│   │   ├── Header.stories.ts
│   │   └── Page.stories.ts
│   └── index.ts                   # Public API
├── .storybook/                    # Storybook configuration
│   ├── main.ts                    # Storybook config
│   ├── preview.ts                 # Global decorators
│   └── vitest.setup.ts            # Vitest setup
├── package.json
├── vite.config.ts
└── tsconfig.json
```

### 4. Component Export Strategy

**Public API (`src/index.ts`):**
```typescript
export * from "./components";
```

**Component Index (`src/components/index.ts`):**
```typescript
export { default as Dialog } from "./ui/Dialog.vue";
export { default as Button } from "./ui/Button.vue";
export { default as Input } from "./ui/Input.vue";
export { default as Label } from "./ui/Label.vue";
export { default as CreateRepositoryDialog } from "./CreateRepositoryDialog.vue";
export { default as LinkRepositoryDialog } from "./LinkRepositoryDialog.vue";
```

**Usage in Apps:**
```vue
<script setup lang="ts">
import { Button, Dialog } from 'design';
</script>
```

### 5. Storybook Configuration

**Addons Enabled:**
- `@chromatic-com/storybook`: Visual regression testing
- `@storybook/addon-docs`: Automatic documentation
- `@storybook/addon-a11y`: Accessibility auditing
- `@storybook/addon-vitest`: Component testing integration

**Story Format:**
```typescript
// Button.stories.ts
import type { Meta, StoryObj } from '@storybook/vue3';
import Button from './Button.vue';

const meta = {
  title: 'UI/Button',
  component: Button,
  tags: ['autodocs'],
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    label: 'Click me',
    variant: 'primary',
  },
};
```

### 6. Development Workflow

**Local Development:**
```bash
bun run storybook    # Start Storybook on :6006
```
- Hot module reloading
- Interactive component explorer
- Instant feedback on changes

**Building:**
```bash
bun run build           # Build component library
bun run build-storybook # Build static Storybook
```

**Testing:**
```bash
bun run test           # Run component tests
```
- Vitest with browser mode
- Component unit tests
- Accessibility tests

### 7. Design Tokens

**Visual Style:**
- Inspired by shadcn/ui design language
- Minimal, clean aesthetic
- Focus on usability over decoration
- Consistent spacing and typography

**Not Using:**
- No CSS framework dependency (components are self-styled)
- No design tokens file (future enhancement)
- No theming system yet (can be added later)

## Consequences

### Positive

- **Consistency**: Centralized components ensure uniform UI
- **Reusability**: Components shared across core app and extensions
- **Documentation**: Storybook provides interactive component docs
- **Isolation**: Develop and test components independently
- **Accessibility**: Addon-a11y helps catch accessibility issues early
- **Type Safety**: TypeScript + Vue 3 provides excellent type checking
- **Fast Development**: Pre-built primitives speed up feature development

### Negative

- **Maintenance Overhead**: Design system requires ongoing maintenance
- **Learning Curve**: Developers must learn Storybook and component API
- **Build Complexity**: Additional package in monorepo adds build steps
- **No Theming**: Lack of design tokens limits customization
- **Limited Components**: Small component library (only 6 components currently)

### Trade-offs

- **Custom vs shadcn-vue**: Built custom components instead of adopting shadcn-vue
  - Rationale: Full control over implementation, no external dependency
  - Future: Can migrate to shadcn-vue if needed
- **Storybook vs No Docs**: Chose Storybook despite overhead
  - Rationale: Interactive docs essential for component library
- **Vue 3 vs Framework-Agnostic**: Chose Vue 3-specific components
  - Rationale: Project uses Vue; no need for multi-framework support
- **Minimal vs Comprehensive**: Built only necessary components
  - Rationale: Start small, expand as needed

## Implementation Details

### Component Patterns

**Props Interface:**
```typescript
interface ButtonProps {
  label: string;
  variant?: 'primary' | 'secondary';
  size?: 'small' | 'medium' | 'large';
  disabled?: boolean;
}
```

**Events:**
```typescript
const emit = defineEmits<{
  click: [event: MouseEvent];
  'update:modelValue': [value: string];
}>();
```

**Slots:**
```vue
<slot name="actions">
  <Button @click="emit('update:open', false)">Close</Button>
</slot>
```

### Accessibility

- Semantic HTML elements (`<button>`, `<label>`, `<input>`)
- ARIA attributes (`role`, `aria-label`, `aria-describedby`)
- Keyboard navigation support
- Focus management in dialogs
- Screen reader compatibility

### File Naming

- Components: PascalCase (`Button.vue`, `CreateRepositoryDialog.vue`)
- Stories: `ComponentName.stories.ts`
- Exports: Named exports for tree shaking

## Future Enhancements

1. **Design Tokens**: Add CSS variables for colors, spacing, typography
2. **Theming**: Support light/dark mode with theme provider
3. **More Components**: Add Table, Card, Tabs, Dropdown, Toast, etc.
4. **Form Validation**: Integrate validation library (Vee-Validate, Valibot)
5. **Animation**: Add transitions and micro-interactions
6. **Icons**: Integrate icon library (Heroicons, Lucide)
7. **Responsive**: Improve mobile responsiveness
8. **Testing**: Expand component test coverage
9. **Documentation**: Add usage guidelines and best practices
10. **Chromatic Integration**: Set up visual regression testing in CI

## References

- Vue 3 documentation: https://vuejs.org
- Storybook documentation: https://storybook.js.org
- Component source: `design/src/components/`
- Stories: `design/src/stories/`
- Storybook config: `design/.storybook/main.ts`
