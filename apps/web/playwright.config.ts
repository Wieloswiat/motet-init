import { defineConfig } from '@playwright/test';

// See vite.config.ts: the dev shell provides Chromium.
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;

export default defineConfig({
	webServer: { command: 'pnpm build && pnpm preview', port: 4173 },
	testDir: 'e2e',
	testMatch: '**/*.e2e.ts',
	use: { launchOptions: { executablePath } }
});
