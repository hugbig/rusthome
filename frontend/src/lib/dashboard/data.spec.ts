import { describe, expect, it } from 'vitest';
import { groupByDay, mediaItems, parseEvent } from './data';
import { centerOf, fitZoom, pointIn, project, tilesFor } from './geo';

describe('médias (upcoming media)', () => {
	const data = JSON.stringify([
		{
			title_default: '$title',
			line1_default: '$release',
			line2_default: 'Runtime: $runtime',
			line3_default: '$genres'
		},
		{
			title: 'Film A',
			release: '2026',
			runtime: 4707,
			genres: 'Action',
			poster: 'http://192.0.2.21/a.jpg'
		},
		{ title: 'Film B', release: '2025' },
		{ notitle: true }
	]);

	it('lit la liste sérialisée, remplit les lignes, ignore la durée', () => {
		const items = mediaItems(data);
		expect(items).toHaveLength(2);
		expect(items[0]).toEqual({
			title: 'Film A',
			lines: ['2026', 'Action'],
			index: 0,
			hasImage: true
		});
		expect(items[1]).toEqual({ title: 'Film B', lines: ['2025'], index: 1, hasImage: false });
	});

	it('tolère les données absentes ou cassées', () => {
		expect(mediaItems(undefined)).toEqual([]);
		expect(mediaItems('pas du json')).toEqual([]);
	});
});

describe('agenda', () => {
	it('distingue journée entière et heure précise', () => {
		const day = parseEvent({ summary: 'Férié', start: { date: '2026-11-01' } }, 'calendar.f')!;
		expect(day.allDay).toBe(true);
		expect(new Date(day.start).getDate()).toBe(1);
		const at = parseEvent(
			{ summary: 'Piscine', start: { dateTime: '2026-10-18T10:00:00+02:00' } },
			'c'
		)!;
		expect(at.allDay).toBe(false);
		expect(at.start).toBe(Date.parse('2026-10-18T08:00:00Z'));
		expect(parseEvent({ summary: 'x' }, 'c')).toBeNull();
	});

	it('groupe par jour dans l’ordre', () => {
		const ev = (s: string) => parseEvent({ summary: s, start: { dateTime: s } }, 'c')!;
		const groups = groupByDay([
			ev('2026-10-19T09:00:00'),
			ev('2026-10-18T20:00:00'),
			ev('2026-10-18T08:00:00')
		]);
		expect(groups.map((g) => g.events.length)).toEqual([2, 1]);
		expect(groups[0]!.events[0]!.summary).toBe('2026-10-18T08:00:00');
	});
});

describe('carte', () => {
	it('projette en Web Mercator', () => {
		const p = project({ lat: 0, lon: 0 }, 0);
		expect(p).toEqual({ x: 128, y: 128 });
	});

	it('centre le point et couvre la fenêtre de tuiles', () => {
		const c = { lat: 48.79, lon: 2.3 };
		expect(pointIn(c, c, 15, 400, 260)).toEqual({ left: 200, top: 130 });
		const tiles = tilesFor(c, 15, 400, 260);
		expect(tiles.length).toBeGreaterThanOrEqual(4);
		expect(tiles.every((t) => t.left > -256 && t.top > -256 && t.left < 400 && t.top < 260)).toBe(
			true
		);
	});

	it('dézoome pour faire tenir plusieurs points', () => {
		const pts = [
			{ lat: 48.79, lon: 2.3 },
			{ lat: 48.86, lon: 2.35 }
		];
		const z = fitZoom(pts, 400, 260);
		expect(z).toBeLessThan(15);
		expect(centerOf(pts).lat).toBeCloseTo(48.825);
	});
});
