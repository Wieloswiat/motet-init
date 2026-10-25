import { defineConfig } from 'vite';

// Two pages: the editor, and the preview it loads in an iframe.
export default defineConfig({
	build: {
		rollupOptions: {
			input: { index: 'index.html', frame: 'frame.html' }
		}
	}
});
