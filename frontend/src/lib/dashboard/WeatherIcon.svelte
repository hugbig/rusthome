<script lang="ts">
	// Icône météo illustrée (soleil, lune, nuages, pluie, neige, orage, brume,
	// vent), composée en SVG pour chaque condition de HA.
	let { condition, size = 64 }: { condition: string; size?: number } = $props();

	const uid = `w${Math.random().toString(36).slice(2, 8)}`;
	const c = $derived(condition);
	const sun = $derived(c === 'sunny' || c === 'partlycloudy' || c === 'windy-variant');
	const moon = $derived(c === 'clear-night');
	const cloud = $derived(!['sunny', 'clear-night', 'windy', 'exceptional'].includes(c));
	const dark = $derived(
		['rainy', 'pouring', 'lightning', 'lightning-rainy', 'snowy-rainy', 'hail'].includes(c)
	);
	// avec un soleil derrière, le nuage est plus petit et décalé
	const small = $derived(c === 'partlycloudy' || c === 'windy-variant');
	const rain = $derived(['rainy', 'pouring', 'lightning-rainy', 'snowy-rainy'].includes(c));
	const heavy = $derived(c === 'pouring');
	const snow = $derived(c === 'snowy' || c === 'snowy-rainy');
	const bolt = $derived(c === 'lightning' || c === 'lightning-rainy');
	const hail = $derived(c === 'hail');
	const fog = $derived(c === 'fog');
	const wind = $derived(c === 'windy' || c === 'windy-variant');
</script>

<svg viewBox="0 0 64 64" width={size} height={size} role="img" aria-hidden="true">
	<defs>
		<radialGradient id="{uid}-sun" cx="0.4" cy="0.35" r="0.7">
			<stop offset="0" stop-color="#ffe082" />
			<stop offset="1" stop-color="#ffb300" />
		</radialGradient>
		<linearGradient id="{uid}-moon" x1="0" y1="0" x2="1" y2="1">
			<stop offset="0" stop-color="#fff3c4" />
			<stop offset="1" stop-color="#e6c25a" />
		</linearGradient>
		<linearGradient id="{uid}-cloud" x1="0" y1="0" x2="0" y2="1">
			<stop offset="0" stop-color="#f4f7fb" />
			<stop offset="1" stop-color="#c7d1df" />
		</linearGradient>
		<linearGradient id="{uid}-dark" x1="0" y1="0" x2="0" y2="1">
			<stop offset="0" stop-color="#b3bfcf" />
			<stop offset="1" stop-color="#7b8aa0" />
		</linearGradient>
	</defs>

	{#if sun}
		<g transform={small ? 'translate(-8 -8) scale(0.9)' : ''}>
			<g stroke="#ffc107" stroke-width="3" stroke-linecap="round">
				{#each [0, 45, 90, 135, 180, 225, 270, 315] as a (a)}
					<line x1="32" y1="9" x2="32" y2="14" transform="rotate({a} 32 32)" />
				{/each}
			</g>
			<circle cx="32" cy="32" r="12" fill="url(#{uid}-sun)" />
		</g>
	{/if}
	{#if moon}
		<path
			d="M38 12a20 20 0 1 0 14 34A17 17 0 0 1 38 12z"
			fill="url(#{uid}-moon)"
			transform="translate(-4 2)"
		/>
	{/if}
	{#if cloud}
		<g transform={small ? 'translate(8 8) scale(0.82)' : 'translate(0 -2)'}>
			<path
				d="M19 48h28a11 11 0 0 0 0-22 15 15 0 0 0-28.6 4.5A8.8 8.8 0 0 0 19 48z"
				fill="url(#{uid}-{dark ? 'dark' : 'cloud'})"
			/>
		</g>
	{/if}
	{#if rain}
		<g stroke="#4fa8f0" stroke-width="3" stroke-linecap="round">
			<line x1="24" y1="51" x2="21" y2="58" />
			<line x1="34" y1="51" x2="31" y2="58" />
			{#if heavy}<line x1="44" y1="51" x2="41" y2="58" />{/if}
			{#if heavy}<line x1="29" y1="57" x2="27" y2="62" />{/if}
		</g>
	{/if}
	{#if snow}
		<g fill="#e3f2fd" stroke="#90caf9" stroke-width="0.8">
			<circle cx="26" cy="55" r="2.6" />
			<circle cx="38" cy="57" r="2.6" />
			{#if c === 'snowy'}<circle cx="32" cy="61" r="2.2" />{/if}
		</g>
	{/if}
	{#if hail}
		<g fill="#d6e6f5">
			<circle cx="24" cy="55" r="2.6" />
			<circle cx="33" cy="58" r="2.6" />
			<circle cx="42" cy="55" r="2.6" />
		</g>
	{/if}
	{#if bolt}
		<path
			d="M35 44l-8 11h6l-3 9 10-13h-6l3-7z"
			fill="#ffca28"
			stroke="#f9a825"
			stroke-width="0.8"
		/>
	{/if}
	{#if fog}
		<g stroke="#a7b3c4" stroke-width="3" stroke-linecap="round">
			<line x1="14" y1="52" x2="50" y2="52" />
			<line x1="20" y1="58" x2="44" y2="58" />
		</g>
	{/if}
	{#if wind}
		<g fill="none" stroke="#9fb3c8" stroke-width="3" stroke-linecap="round">
			<path d="M10 {small ? 54 : 26}h30a6 6 0 1 0-6-6" />
			<path d="M10 {small ? 60 : 36}h40a6 6 0 1 1-6 6" />
		</g>
	{/if}
	{#if c === 'exceptional'}
		<circle cx="32" cy="32" r="18" fill="none" stroke="#e66767" stroke-width="4" />
		<line
			x1="32"
			y1="22"
			x2="32"
			y2="34"
			stroke="#e66767"
			stroke-width="4"
			stroke-linecap="round"
		/>
		<circle cx="32" cy="41" r="2.5" fill="#e66767" />
	{/if}
</svg>

<style>
	svg {
		display: block;
		flex: none;
		filter: drop-shadow(0 2px 4px rgb(0 0 0 / 0.15));
	}
</style>
