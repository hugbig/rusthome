<script lang="ts">
	// Météo façon HA : grande icône illustrée, température, détails, puis les
	// prévisions des prochains jours (lues dans HA, mises en cache 10 min).
	import { request } from '../../api';
	import { live } from '../../stores/live.svelte';
	import Icon from '../Icon.svelte';
	import { mdiGauge, mdiWaterPercent, mdiWeatherWindy } from '@mdi/js';
	import { cardName, formatState, stateLabel, type Card } from '../model';
	import WeatherIcon from '../WeatherIcon.svelte';

	let { card }: { card: Card } = $props();

	interface Forecast {
		datetime: string;
		condition?: string;
		temperature?: number;
		templow?: number;
		precipitation?: number;
		precipitation_probability?: number;
	}

	const s = $derived(card.entity ? live.states.get(card.entity) : undefined);
	const num = (v: unknown) => (typeof v === 'number' ? v : undefined);
	const temp = $derived(num(s?.attributes.temperature));
	const unit = $derived(
		typeof s?.attributes.temperature_unit === 'string' ? s.attributes.temperature_unit : '°C'
	);
	const fmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 });
	const round = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 0 });
	const EXTRA_LABELS: Record<string, string> = {
		rain_chance: 'Pluie',
		next_rain: 'Prochaine pluie',
		uv: 'UV',
		alert: 'Vigilance',
		cloud_cover: 'Nuages',
		freeze_chance: 'Gel',
		snow_chance: 'Neige'
	};
	const extra = $derived(
		Object.entries((card.extra ?? {}) as Record<string, string>).filter(
			([, e]) => typeof e === 'string'
		)
	);
	const details = $derived(
		[
			[mdiWaterPercent, 'Humidité', num(s?.attributes.humidity), '%'],
			[
				mdiWeatherWindy,
				'Vent',
				num(s?.attributes.wind_speed),
				String(s?.attributes.wind_speed_unit ?? 'km/h')
			],
			[
				mdiGauge,
				'Pression',
				num(s?.attributes.pressure),
				String(s?.attributes.pressure_unit ?? 'hPa')
			]
		].filter(([, , v]) => v !== undefined) as [string, string, number, string][]
	);

	let forecast = $state<Forecast[]>([]);
	$effect(() => {
		const id = card.entity;
		if (!id || card.show_forecast === false) return;
		void request<Forecast[]>('GET', `weather/${encodeURIComponent(id)}/forecast?type=daily`).then(
			(r) => (forecast = r.ok && Array.isArray(r.data) ? r.data : [])
		);
	});
	const today = $derived(forecast[0]);
	const days = $derived(forecast.slice(1, 6));
	const dayFmt = new Intl.DateTimeFormat('fr-FR', { weekday: 'short' });
	const title = $derived(cardName(card, s));
</script>

<div class="dcard weather">
	<div class="now">
		<WeatherIcon condition={s?.state ?? 'cloudy'} size={84} />
		<div class="txt">
			<div class="cond">{s ? stateLabel(s.state) : 'Introuvable'}</div>
			<div class="place" {title}>{title}</div>
		</div>
		<div class="temps">
			<div class="temp">
				{temp !== undefined ? fmt.format(temp) : '—'}<span class="u">{unit}</span>
			</div>
			{#if today?.temperature !== undefined}
				<div class="hilo">
					{round.format(today.temperature)}° / {today.templow !== undefined
						? `${round.format(today.templow)}°`
						: '—'}
				</div>
			{/if}
		</div>
	</div>

	{#if details.length || extra.length}
		<div class="pills">
			{#each details as [icon, label, v, u] (label)}
				<span class="pill" title={label}>
					<Icon path={icon} size={16} />
					{fmt.format(v)}&nbsp;{u}
				</span>
			{/each}
			{#each extra as [key, e] (key)}
				<span class="pill" title={EXTRA_LABELS[key] ?? key}>
					<span class="k">{EXTRA_LABELS[key] ?? key}</span>
					{formatState(live.states.get(e))}
				</span>
			{/each}
		</div>
	{/if}

	{#if days.length}
		<div class="days" style:--n={days.length}>
			{#each days as d (d.datetime)}
				<div class="day">
					<span class="dname">{dayFmt.format(new Date(d.datetime)).replace('.', '')}</span>
					<WeatherIcon condition={d.condition ?? 'cloudy'} size={36} />
					<span class="hi"
						>{d.temperature !== undefined ? `${round.format(d.temperature)}°` : '—'}</span
					>
					<span class="lo">{d.templow !== undefined ? `${round.format(d.templow)}°` : ''}</span>
					{#if (d.precipitation ?? 0) > 0}
						<span class="rain">{fmt.format(d.precipitation!)} mm</span>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.weather {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px;
	}
	.now {
		display: flex;
		align-items: center;
		gap: 14px;
	}
	.txt {
		flex: 1;
		min-width: 0;
	}
	.cond {
		font-size: 1.15rem;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.place {
		color: var(--text-muted);
		font-size: 0.85rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.temps {
		text-align: right;
	}
	.temp {
		font-size: 2.4rem;
		font-weight: 600;
		line-height: 1;
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
	}
	.u {
		font-size: 1rem;
		font-weight: 500;
		color: var(--text-muted);
		margin-left: 2px;
		vertical-align: top;
	}
	.hilo {
		margin-top: 4px;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.pills {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: 999px;
		background: var(--surface-2);
		font-size: 0.82rem;
		color: var(--text);
	}
	.pill :global(svg) {
		color: var(--text-muted);
	}
	.k {
		color: var(--text-muted);
	}
	.days {
		display: grid;
		grid-template-columns: repeat(var(--n), minmax(0, 1fr));
		gap: 4px;
		padding-top: 12px;
		border-top: 1px solid var(--border);
	}
	.day {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		font-variant-numeric: tabular-nums;
	}
	.dname {
		font-size: 0.78rem;
		color: var(--text-muted);
		text-transform: capitalize;
	}
	.hi {
		font-weight: 600;
		font-size: 0.9rem;
	}
	.lo {
		color: var(--text-muted);
		font-size: 0.82rem;
	}
	.rain {
		color: #4fa8f0;
		font-size: 0.72rem;
	}
</style>
