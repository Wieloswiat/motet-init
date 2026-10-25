export default {
	// Stryker 10.1.1 applies Svelte ranges inside each script, before adding
	// the script's file offset. Local lines 5-10 cover the bindable selection
	// and change handler; Panda's static styles start at local line 12.
	// The module's `as const` catalogue is skipped as a TypeScript type node.
	// Template expressions are outside this owned-script scope.
	mutate: ['src/lib/components/ThemeSwitch.svelte:5-10'],
	// The Vitest runner does not support Browser Mode. Keep the real browser
	// and let the command runner consume the complete client suite's exit code.
	testRunner: 'command',
	coverageAnalysis: 'off',
	commandRunner: { command: 'pnpm exec vitest run --project client --bail 1' },
	// Stryker does not use a shell here; the package script sequences both generators.
	buildCommand: 'pnpm run prepare',
	concurrency: 1,
	// SvelteKit 3 sync generates sandbox-local node_modules/$app aliases before
	// Stryker links dependencies. Keep that directory private; normal Node/Vite
	// resolution finds the installed packages in the ancestor app node_modules.
	symlinkNodeModules: false,
	ignorePatterns: [
		'build/**',
		'.svelte-kit/**',
		'styled-system/**',
		'reports/**',
		'playwright-report/**',
		'test-results/**',
		'e2e/**'
	],
	reporters: ['clear-text', 'progress', 'html', 'json'],
	htmlReporter: { fileName: 'reports/mutation/index.html' },
	jsonReporter: { fileName: 'reports/mutation/mutation.json' },
	thresholds: { high: 100, low: 100, break: 100 }
};
