import { readFileSync } from 'node:fs';

const report = JSON.parse(readFileSync(process.argv[2], 'utf8'));
let killed = 0;

// Stryker counts timeouts toward its score and excludes runtime errors, so even
// a 100% score is not evidence that tests caught every viable mutant.
for (const [file, result] of Object.entries(report.files)) {
	for (const mutant of result.mutants) {
		if (mutant.status === 'Killed') {
			killed += 1;
		} else if (mutant.status !== 'CompileError' && mutant.status !== 'Ignored') {
			console.error(
				`${file}:${JSON.stringify(mutant.location)} id=${mutant.id} status=${mutant.status}`
			);
			process.exitCode = 1;
		}
	}
}

if (killed === 0) {
	console.error(
		'Mutation report contains no Killed mutant; no caught behavioral mutation was demonstrated.'
	);
	process.exitCode = 1;
}
