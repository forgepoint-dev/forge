import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: './tests/playwright',
	testMatch: '**/*.e2e.ts',  // Match E2E test files
	fullyParallel: true,
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	workers: process.env.CI ? 1 : undefined,
	reporter: 'html',
	use: {
		baseURL: 'http://localhost:4321',
		trace: 'on-first-retry',
	},
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] },
		},
	],
	webServer: {
		command: 'bun run dev',
		url: 'http://localhost:4321',
		reuseExistingServer: !process.env.CI,
	},
});
