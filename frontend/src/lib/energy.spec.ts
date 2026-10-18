import { describe, expect, it } from 'vitest';
import {
	aggregate,
	changePct,
	gridPricing,
	bucketOf,
	clip,
	energyConfig,
	fetchPeriod,
	mergeSeries,
	periodRange,
	slots,
	total
} from './energy';

describe('énergie', () => {
	it('lit les réglages de HA (nouveau et ancien format)', () => {
		expect(
			energyConfig({
				energy_sources: [
					{ type: 'grid', stat_energy_from: 'sensor.z_linky_base', stat_energy_to: null }
				],
				device_consumption: [{ stat_consumption: 'sensor.radiateur' }]
			})
		).toEqual({ grid: ['sensor.z_linky_base'], gridReturn: [], devices: ['sensor.radiateur'] });
		expect(
			energyConfig({
				energy_sources: [
					{
						type: 'grid',
						flow_from: [{ stat_energy_from: 'sensor.hp' }, { stat_energy_from: 'sensor.hc' }],
						flow_to: [{ stat_energy_to: 'sensor.injection' }]
					},
					{ type: 'solar', stat_energy_from: 'sensor.solaire' }
				]
			}).grid
		).toEqual(['sensor.hp', 'sensor.hc']);
		expect(energyConfig(null)).toEqual({ grid: [], gridReturn: [], devices: [] });
	});

	it('calcule les périodes (semaine du lundi, mois, année)', () => {
		const ref = new Date(2026, 9, 17, 15, 0); // samedi 17 octobre
		const [ds, de] = periodRange('day', -1, ref);
		expect([ds.getDate(), de.getDate()]).toEqual([16, 17]);
		const [ws, we] = periodRange('week', 0, ref);
		expect([ws.getDay(), ws.getDate(), we.getDate()]).toEqual([1, 12, 19]);
		const [ms, me] = periodRange('month', 1, ref);
		expect([ms.getMonth(), me.getMonth()]).toEqual([10, 11]);
		const [ys] = periodRange('year', 0, ref);
		expect([ys.getFullYear(), ys.getMonth(), ys.getDate()]).toEqual([2026, 0, 1]);
		expect([bucketOf('day'), bucketOf('week'), bucketOf('year')]).toEqual(['hour', 'day', 'month']);
	});

	it('additionne compteurs et créneaux', () => {
		const a = [
			{ start: 1, change: 1.5 },
			{ start: 2, change: 2 }
		];
		const b = [
			{ start: 2, change: 0.5 },
			{ start: 3, change: null }
		];
		expect(total(a)).toBe(3.5);
		expect(mergeSeries([a, b])).toEqual([
			{ start: 1, value: 1.5 },
			{ start: 2, value: 2.5 },
			{ start: 3, value: 0 }
		]);
	});

	it('écarte le créneau de la borne de fin', () => {
		const start = new Date(2026, 9, 5);
		const end = new Date(2026, 9, 12);
		const rows = [
			{ start: start.getTime(), change: 1 },
			{ start: end.getTime() - 1, change: 1 },
			{ start: end.getTime(), change: 9 }
		];
		expect(total(clip(rows, start, end))).toBe(2);
	});

	it('regroupe les statistiques sur 5 minutes par heure', () => {
		const h0 = new Date(2026, 9, 18, 0, 0).getTime();
		const rows = [
			{ start: h0, change: 0.1 },
			{ start: h0 + 5 * 60_000, change: 0.2 },
			{ start: h0 + 60 * 60_000, change: 0.5 }
		];
		const hours = aggregate(rows, 'hour');
		expect(hours.map((r) => r.start)).toEqual([h0, h0 + 3600_000]);
		expect(hours[0]!.change).toBeCloseTo(0.3);
		expect(slots(new Date(2026, 9, 18), new Date(2026, 9, 19), 'hour')).toHaveLength(24);
		expect(slots(new Date(2026, 0, 1), new Date(2027, 0, 1), 'month')).toHaveLength(12);
		expect([fetchPeriod('day', 0), fetchPeriod('day', -30), fetchPeriod('month', 0)]).toEqual([
			'5minute',
			'hour',
			'day'
		]);
	});

	it('lit le tarif et compare les périodes', () => {
		expect(
			gridPricing({
				energy_sources: [{ type: 'grid', stat_energy_from: 'a', number_energy_price: 0.2516 }]
			})
		).toEqual({ costStats: [], price: 0.2516 });
		expect(
			gridPricing({ energy_sources: [{ type: 'grid', stat_cost: 'sensor.cout' }] }).costStats
		).toEqual(['sensor.cout']);
		expect(changePct(110, 100)).toBe(10);
		expect(changePct(5, 0)).toBeUndefined();
	});
});
