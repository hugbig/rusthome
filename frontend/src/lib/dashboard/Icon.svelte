<script lang="ts">
	let { path, size = 24, label }: { path: string; size?: number; label?: string } = $props();
</script>

<svg
	viewBox="0 0 24 24"
	width={size}
	height={size}
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	<path d={path} fill="currentColor" />
</svg>

<style>
	svg {
		flex: none;
		display: block;
	}
</style>
