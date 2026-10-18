<script lang="ts">
	// Commande d'une lumière, comme la fiche de HA : grand curseur de luminosité
	// vertical, roue chromatique, blanc réglable. Les commandes partent au
	// relâchement (pas une rafale pendant le glissé).
	import { mdiLightbulbOutline, mdiPalette, mdiPower, mdiThermometer } from '@mdi/js';
	import type { EntityState } from '../types';
	import Icon from './Icon.svelte';
	import { hsToWheel, lightCaps, wheelToHs } from './light';
	import { entityColor, kelvinToRgb, LIGHT_ON } from './model';
	import { call } from './runtime.svelte';

	let { s }: { s: EntityState } = $props();

	const caps = $derived(lightCaps(s));
	const on = $derived(s.state === 'on');
	const color = $derived(entityColor(s) ?? LIGHT_ON);
	const serverPct = $derived(
		on && typeof s.attributes.brightness === 'number'
			? Math.round((s.attributes.brightness / 255) * 100)
			: 0
	);
	let dragPct = $state<number | null>(null);
	const pct = $derived(dragPct ?? serverPct);
	let mode = $state<'color' | 'temp'>('color');
	$effect(() => {
		if (!caps.color && caps.temp) mode = 'temp';
	});

	const id = $derived(s.entity_id);
	const turnOn = (data: Record<string, unknown>) =>
		call('light.turn_on', { entity_id: id, ...data });

	// --- luminosité (glisser vertical) ---
	let bar = $state<HTMLDivElement>();
	function pctAt(e: PointerEvent): number {
		const r = bar!.getBoundingClientRect();
		return Math.round(Math.min(1, Math.max(0, (r.bottom - e.clientY) / r.height)) * 100);
	}
	function barDown(e: PointerEvent) {
		bar?.setPointerCapture(e.pointerId);
		dragPct = pctAt(e);
	}
	function barMove(e: PointerEvent) {
		if (dragPct !== null) dragPct = pctAt(e);
	}
	async function barUp() {
		if (dragPct === null) return;
		const v = dragPct;
		await (v === 0 ? call('light.turn_off', { entity_id: id }) : turnOn({ brightness_pct: v }));
		dragPct = null;
	}
	function barKey(e: KeyboardEvent) {
		const step = e.key === 'ArrowUp' ? 5 : e.key === 'ArrowDown' ? -5 : 0;
		if (!step) return;
		e.preventDefault();
		const v = Math.min(100, Math.max(0, serverPct + step));
		void (v === 0 ? call('light.turn_off', { entity_id: id }) : turnOn({ brightness_pct: v }));
	}

	// --- roue chromatique ---
	const WHEEL = 220;
	const R = WHEEL / 2;
	let wheel = $state<HTMLDivElement>();
	let dragHs = $state<[number, number] | null>(null);
	const hs = $derived.by((): [number, number] => {
		if (dragHs) return dragHs;
		const v = s.attributes.hs_color;
		return Array.isArray(v) && v.length === 2 ? [Number(v[0]), Number(v[1])] : [0, 0];
	});
	const marker = $derived(hsToWheel(hs[0], hs[1], R - 8));
	function hsAt(e: PointerEvent): [number, number] {
		const r = wheel!.getBoundingClientRect();
		return wheelToHs(e.clientX - r.left - R, e.clientY - r.top - R, R - 8);
	}
	function wheelDown(e: PointerEvent) {
		wheel?.setPointerCapture(e.pointerId);
		dragHs = hsAt(e);
	}
	function wheelMove(e: PointerEvent) {
		if (dragHs) dragHs = hsAt(e);
	}
	async function wheelUp() {
		if (!dragHs) return;
		const v = dragHs;
		await turnOn({ hs_color: v });
		dragHs = null;
	}

	// --- blanc réglable ---
	const kMin = $derived(Number(s.attributes.min_color_temp_kelvin ?? 2000));
	const kMax = $derived(Number(s.attributes.max_color_temp_kelvin ?? 6500));
	let dragK = $state<number | null>(null);
	const kelvin = $derived(dragK ?? Number(s.attributes.color_temp_kelvin ?? (kMin + kMax) / 2));
	const rgb = (k: number) => `rgb(${kelvinToRgb(k).join(' ')})`;
	const tempGradient = $derived(
		`linear-gradient(to right, ${[0, 0.25, 0.5, 0.75, 1].map((f) => rgb(kMin + f * (kMax - kMin))).join(', ')})`
	);
</script>

<div class="light">
	{#if caps.dim}
		<div
			class="bar"
			bind:this={bar}
			role="slider"
			tabindex="0"
			aria-label="Luminosité"
			aria-valuemin="0"
			aria-valuemax="100"
			aria-valuenow={pct}
			onpointerdown={barDown}
			onpointermove={barMove}
			onpointerup={barUp}
			onpointercancel={() => (dragPct = null)}
			onkeydown={barKey}
		>
			<div class="fill" style:height="{pct}%" style:background={color}></div>
			<span class="pct">{on || dragPct !== null ? `${pct} %` : 'Éteinte'}</span>
			<span class="bulb"><Icon path={mdiLightbulbOutline} size={22} /></span>
		</div>
	{/if}

	<div class="side">
		<button
			class="power"
			class:on
			style:--c={color}
			onclick={() => call(on ? 'light.turn_off' : 'light.turn_on', { entity_id: id })}
			aria-label={on ? 'Éteindre' : 'Allumer'}
		>
			<Icon path={mdiPower} size={24} />
			{on ? 'Éteindre' : 'Allumer'}
		</button>

		{#if caps.color && caps.temp}
			<div class="modes" role="tablist">
				<button role="tab" aria-selected={mode === 'color'} onclick={() => (mode = 'color')}>
					<Icon path={mdiPalette} size={18} /> Couleur
				</button>
				<button role="tab" aria-selected={mode === 'temp'} onclick={() => (mode = 'temp')}>
					<Icon path={mdiThermometer} size={18} /> Blanc
				</button>
			</div>
		{/if}

		{#if caps.color && mode === 'color'}
			<div
				class="wheel"
				bind:this={wheel}
				style:width="{WHEEL}px"
				style:height="{WHEEL}px"
				role="slider"
				tabindex="0"
				aria-label="Couleur (teinte {hs[0]}°, saturation {hs[1]} %)"
				aria-valuenow={hs[0]}
				onpointerdown={wheelDown}
				onpointermove={wheelMove}
				onpointerup={wheelUp}
				onpointercancel={() => (dragHs = null)}
			>
				<span
					class="marker"
					style:transform="translate({R + marker[0]}px, {R + marker[1]}px)"
					style:background="hsl({hs[0]} 100% {100 - hs[1] / 2}%)"
				></span>
			</div>
		{:else if caps.temp}
			<label class="temp">
				<span>Blanc : {Math.round(kelvin)} K</span>
				<input
					type="range"
					min={kMin}
					max={kMax}
					step="50"
					value={kelvin}
					style:background={tempGradient}
					class="tslider"
					oninput={(e) => (dragK = Number(e.currentTarget.value))}
					onchange={async (e) => {
						await turnOn({ color_temp_kelvin: Number(e.currentTarget.value) });
						dragK = null;
					}}
				/>
				<span class="ends"><span>Chaud</span><span>Froid</span></span>
			</label>
		{/if}
	</div>
</div>

<style>
	.light {
		display: flex;
		gap: 24px;
		align-items: center;
		justify-content: center;
		flex-wrap: wrap;
		padding: 8px 0 4px;
	}
	.bar {
		position: relative;
		width: 96px;
		height: 300px;
		border-radius: 28px;
		background: var(--surface-3);
		overflow: hidden;
		cursor: ns-resize;
		touch-action: none;
		user-select: none;
	}
	.fill {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		opacity: 0.9;
		transition: height 0.12s;
	}
	.pct {
		position: absolute;
		top: 14px;
		left: 0;
		right: 0;
		text-align: center;
		font-weight: 650;
		font-variant-numeric: tabular-nums;
		mix-blend-mode: difference;
		color: #fff;
	}
	.bulb {
		position: absolute;
		bottom: 12px;
		left: 50%;
		transform: translateX(-50%);
		color: rgb(0 0 0 / 0.55);
	}
	.side {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
	}
	.power {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 10px 18px;
		border: 1px solid var(--border);
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.power.on {
		background: color-mix(in srgb, var(--c) 22%, var(--surface));
		border-color: color-mix(in srgb, var(--c) 45%, transparent);
	}
	.modes {
		display: flex;
		gap: 4px;
		padding: 3px;
		border-radius: 999px;
		background: var(--surface-2);
	}
	.modes button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 14px;
		border: none;
		border-radius: 999px;
		background: none;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}
	.modes button[aria-selected='true'] {
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow);
	}
	.wheel {
		position: relative;
		border-radius: 50%;
		background:
			radial-gradient(circle closest-side, #fff, rgb(255 255 255 / 0) 100%),
			conic-gradient(from 90deg, red, yellow, lime, cyan, blue, magenta, red);
		box-shadow: var(--shadow);
		cursor: crosshair;
		touch-action: none;
	}
	.marker {
		position: absolute;
		top: 0;
		left: 0;
		width: 26px;
		height: 26px;
		margin: -13px 0 0 -13px;
		border-radius: 50%;
		border: 3px solid #fff;
		box-shadow: 0 2px 6px rgb(0 0 0 / 0.4);
		pointer-events: none;
	}
	.temp {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 240px;
		font-size: 0.9rem;
	}
	.tslider {
		height: 16px;
		border-radius: 999px;
	}
	.tslider::-webkit-slider-runnable-track {
		background: transparent;
		height: 16px;
	}
	.tslider::-webkit-slider-thumb {
		margin-top: -1px;
	}
	.tslider::-moz-range-track,
	.tslider::-moz-range-progress {
		background: transparent;
	}
	.ends {
		display: flex;
		justify-content: space-between;
		color: var(--text-muted);
		font-size: 0.78rem;
	}
</style>
