import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		globals: true,
		environment: 'node',
		include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],  // Only include unit tests in src/
		exclude: [
			'**/node_modules/**',
			'**/dist/**',
			'**/*.spec.ts',  // Exclude Playwright spec files
			'**/tests/**',  // Exclude all tests directory
			'**/.{idea,git,cache,output,temp}/**',
		],
	},
});
