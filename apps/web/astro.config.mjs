// @ts-check
import { defineConfig } from "astro/config";
import vue from "@astrojs/vue";
import tailwind from "@astrojs/tailwind";
import { createSlotRegistry, createSlotPlugin } from "./src/lib/slot-plugin.ts";
import node from "@astrojs/node";
// Import issues integration from workspace (for local development)
// In production, use: import issuesIntegration from '@forgepoint/astro-integration-issues';
import issuesIntegration from "../../extensions/issues/ui/src/index.ts";

const slotRegistry = createSlotRegistry();
const slotPlugin = createSlotPlugin(slotRegistry);

export { slotRegistry };

// https://astro.build/config
export default defineConfig({
	integrations: [
		vue(),
		tailwind({
			// We manage base styles via shadcn-style CSS variables
			applyBaseStyles: false,
		}),
		// Enable Issues extension with slot registration
		issuesIntegration({ slotRegistry }),
	],
	// Enable full SSR; server islands work with the Node adapter
	output: "server",
	adapter: node({ mode: "standalone" }),
	// Ensure dev server is reachable on 127.0.0.1 for cookie-domain alignment
	server: {
		host: true,
	},
	preview: {
		host: true,
	},
	vite: {
		plugins: [slotPlugin],
		server: {
			proxy: {
				// Proxy git HTTP protocol requests to the backend API server
				// This allows git clone http://localhost:4321/... to work
				'^/.*/info/refs': {
					target: 'http://localhost:8000',
					changeOrigin: true,
				},
				'^/.*/git-upload-pack': {
					target: 'http://localhost:8000',
					changeOrigin: true,
				},
				'^/.*/git-receive-pack': {
					target: 'http://localhost:8000',
					changeOrigin: true,
				},
			},
		},
	},
});
