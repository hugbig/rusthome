import '@testing-library/jest-dom/vitest';

// jsdom n'implémente pas ResizeObserver (utilisé par bind:clientWidth).
globalThis.ResizeObserver ??= class {
	observe() {}
	unobserve() {}
	disconnect() {}
};
