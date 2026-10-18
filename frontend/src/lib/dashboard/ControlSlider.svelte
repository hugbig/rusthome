<script lang="ts">
	// Curseur de commande des tuiles, repris de ha-control-slider : barre
	// épaisse, remplissage plein, poignée blanche dans le remplissage, bulle
	// de valeur pendant le glissement. La valeur n'est envoyée qu'au relâcher.
	interface Props {
		value: number;
		label: string;
		min?: number;
		max?: number;
		step?: number;
		unit?: string;
		onchange: (value: number) => void;
	}
	let { value, label, min = 0, max = 100, step = 1, unit = '%', onchange }: Props = $props();

	let dragging = $state<number | null>(null);
	let el = $state<HTMLDivElement>();
	const current = $derived(dragging ?? value);
	const ratio = $derived(Math.min(1, Math.max(0, (current - min) / (max - min || 1))));

	function clamp(v: number) {
		const stepped = Math.round((v - min) / step) * step + min;
		return Math.min(max, Math.max(min, stepped));
	}
	function fromPointer(e: PointerEvent) {
		if (!el) return value;
		const r = el.getBoundingClientRect();
		return clamp(min + ((e.clientX - r.left) / r.width) * (max - min));
	}
	function down(e: PointerEvent) {
		el?.setPointerCapture(e.pointerId);
		dragging = fromPointer(e);
	}
	function move(e: PointerEvent) {
		if (dragging !== null) dragging = fromPointer(e);
	}
	function up() {
		if (dragging === null) return;
		const v = dragging;
		dragging = null;
		if (v !== value) onchange(v);
	}
	function key(e: KeyboardEvent) {
		const delta =
			e.key === 'ArrowRight' || e.key === 'ArrowUp'
				? step
				: e.key === 'ArrowLeft' || e.key === 'ArrowDown'
					? -step
					: e.key === 'PageUp'
						? 10 * step
						: e.key === 'PageDown'
							? -10 * step
							: null;
		const target =
			e.key === 'Home' ? min : e.key === 'End' ? max : delta === null ? null : clamp(value + delta);
		if (target === null) return;
		e.preventDefault();
		if (target !== value) onchange(target);
	}
</script>

<div
	bind:this={el}
	class="slider"
	class:dragging={dragging !== null}
	role="slider"
	tabindex="0"
	aria-label={label}
	aria-valuemin={min}
	aria-valuemax={max}
	aria-valuenow={current}
	aria-valuetext="{current} {unit}"
	style:--value={ratio}
	onpointerdown={down}
	onpointermove={move}
	onpointerup={up}
	onpointercancel={up}
	onkeydown={key}
>
	<div class="track"></div>
	<div class="bar"></div>
	{#if dragging !== null}<span class="tip">{current} {unit}</span>{/if}
</div>

<style>
	.slider {
		--thickness: 42px;
		--handle: 4px;
		--margin: calc(var(--thickness) / 8);
		position: relative;
		width: 100%;
		height: var(--thickness);
		border-radius: 12px;
		overflow: hidden;
		cursor: pointer;
		touch-action: none;
		outline: none;
		transform: translateZ(0);
		transition: box-shadow 0.18s ease-in-out;
	}
	.slider:focus-visible {
		box-shadow: 0 0 0 2px var(--switch-on-thumb);
	}
	.track {
		position: absolute;
		inset: 0;
		background: var(--control-track);
		opacity: 0.35;
	}
	/* La barre se décale de (valeur − 1) × largeur utile : la poignée reste
	   toujours visible, même à 0. */
	.bar {
		--size: calc(100% - 2 * var(--margin) - var(--handle));
		position: absolute;
		inset: 0;
		border-radius: 8px;
		/* même couleur que la pastille d'un interrupteur allumé */
		background: var(--switch-on-thumb);
		transform: translate3d(calc((var(--value) - 1) * var(--size)), 0, 0);
		transition: transform 0.18s ease-in-out;
	}
	.dragging .bar {
		transition: none;
	}
	.bar::after {
		content: '';
		position: absolute;
		top: 0;
		bottom: 0;
		right: var(--margin);
		width: var(--handle);
		height: 50%;
		margin: auto;
		border-radius: var(--handle);
		background: #fff;
	}
	.tip {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		padding: 2px 8px;
		border-radius: 8px;
		background: var(--surface);
		color: var(--text);
		font-size: 0.85rem;
		font-weight: 600;
		box-shadow: 0 2px 5px rgb(0 0 0 / 0.2);
		pointer-events: none;
	}
</style>
