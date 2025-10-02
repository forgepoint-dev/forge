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

## Theming

- Auto–dark mode follows the system preference on first load.
- A toggle in the header lets you choose: Auto, Light, or Dark.
- A palette selector lets you switch between Rosé Pine and Catppuccin.
- Theme tokens live in `apps/web/src/styles/global.css` under `data-theme` selectors; Tailwind reads them via `hsl(var(--token))`.
- Preferences persist in `localStorage` under `fp-theme-mode` and `fp-theme-brand`.

Authentication:
- Set `PUBLIC_FORGE_GRAPHQL_URL` (see `.env.example`) to point at the running forge server if it differs from `http://localhost:8000/graphql`.
- Optional: set `PUBLIC_FORGE_AUTH_LOGIN_URL` to the server’s login page (defaults to `{SERVER_BASE}/auth/login` inferred from the GraphQL URL).
- The "Register / Login" button redirects the browser to the server’s ATProto login flow (`/auth/login`). Ensure the server is started with `ATPROTO_CLIENT_ID`, `ATPROTO_CLIENT_SECRET`, and (optionally) `ATPROTO_REDIRECT_URI` so the flow is enabled.

## Build

```
bun run build
```

## Notes

- This setup uses `@astrojs/vue` and Tailwind. If packages are missing, run `bun install`.
- UI primitives in `src/components/ui/` mimic shadcn-vue styles (Button, Input). You can later adopt the full shadcn-vue library if desired.
