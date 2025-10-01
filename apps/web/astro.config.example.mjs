// @ts-check
import { defineConfig } from "astro/config";
import vue from "@astrojs/vue";
import tailwind from "@astrojs/tailwind";
import { createSlotRegistry, createSlotPlugin } from "./src/lib/slot-plugin.ts";
import issuesIntegration from "@forgepoint/astro-integration-issues";

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
		// Example: Enable the issues integration with slot registration
		issuesIntegration({ slotRegistry }),
	],
	vite: {
		plugins: [slotPlugin],
	},
});
