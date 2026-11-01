import { defineConfig } from '@playwright/test';

// See vite.config.ts: the dev shell provides Chromium.
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;

const baseURL = 'https://motet.test';
const previewURL = 'http://127.0.0.1:4174';

export default defineConfig({
	webServer: {
		command: 'pnpm build && pnpm preview --host 127.0.0.1 --port 4174 --strictPort',
		url: previewURL,
		reuseExistingServer: false
	},
	testDir: 'e2e',
	testMatch: '**/*.e2e.ts',
	reporter: [['list'], ['html', { open: 'never' }]],
	use: {
		baseURL,
		colorScheme: 'light',
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure',
		launchOptions: { executablePath }
	}
});
