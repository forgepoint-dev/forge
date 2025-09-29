# Forge Web

Astro + Vue landing page styled with shadcn-like Tailwind tokens. Uses Bun for package management and scripts.

Focus: a Git forge home for a single user or a single organization. Collaboration exists (PRs/issues) but the experience centers your repos, monorepos, and higher-level Products.

## Prerequisites

- Bun 1.1.x (`bun --version`)

## Install

```
bun install
```

## Development

```
bun run dev
```

Open the local URL. The homepage is a Vue island (`src/components/HomeLanding.vue`) rendered within an Astro layout and styled with Tailwind.

Set `PUBLIC_FORGE_GRAPHQL_URL` (see `.env.example`) to point at the running forge server if it differs from the default `http://localhost:8000/graphql`.

## Build

```
bun run build
```

## Notes

- This setup uses `@astrojs/vue` and Tailwind. If packages are missing, run `bun install`.
- UI primitives in `src/components/ui/` mimic shadcn-vue styles (Button, Input). You can later adopt the full shadcn-vue library if desired.
