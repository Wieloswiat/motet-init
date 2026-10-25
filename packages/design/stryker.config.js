/** @type {import('@stryker-mutator/core').PartialStrykerOptions} */
export default {
	mutate: ['src/contrast.ts'],
	testRunner: 'vitest',
	plugins: ['@stryker-mutator/vitest-runner'],
	vitest: { configFile: 'vitest.config.ts' },
	coverageAnalysis: 'perTest',
	concurrency: 2,
	reporters: ['clear-text', 'progress', 'html', 'json'],
	htmlReporter: { fileName: 'reports/mutation/index.html' },
	jsonReporter: { fileName: 'reports/mutation/mutation.json' },
	// This small accessibility oracle must have no unreviewed survivors or
	// uncovered mutants: a broken checker could approve every theme.
	thresholds: { high: 100, low: 100, break: 100 }
};
