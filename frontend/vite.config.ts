import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

// API du backend en dev (cf. AGENTS.md §4.2)
const backend = process.env.RUSTHOME_BACKEND ?? 'http://127.0.0.1:8124';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// SPA servie par le binaire rusthome : tout retombe sur index.html
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		proxy: {
			// Host conservé : l'API refuse les WebSocket d'une autre origine.
			'/api': { target: backend, ws: true, changeOrigin: false }
		}
	},
	resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
	test: {
		expect: { requireAssertions: true },
		environment: 'jsdom',
		include: ['src/**/*.{test,spec}.{js,ts}'],
		setupFiles: ['./vitest-setup.ts']
	}
});
