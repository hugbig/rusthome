import { describe, expect, it } from 'vitest';
import { hsToWheel, lightCaps, wheelToHs } from './light';
import type { EntityState } from '../types';

const st = (attributes: Record<string, unknown>) =>
	({ entity_id: 'light.a', state: 'on', attributes }) as unknown as EntityState;

describe('lumières', () => {
	it('convertit la roue chromatique dans les deux sens', () => {
		expect(wheelToHs(100, 0, 100)).toEqual([0, 100]);
		expect(wheelToHs(0, 50, 100)).toEqual([90, 50]);
		expect(wheelToHs(0, 0, 100)).toEqual([0, 0]);
		expect(wheelToHs(-300, 0, 100)).toEqual([180, 100]);
		const [x, y] = hsToWheel(270, 100, 100);
		expect([Math.round(x) + 0, Math.round(y)]).toEqual([0, -100]);
	});

	it('déduit les commandes possibles', () => {
		expect(lightCaps(st({ supported_color_modes: ['color_temp', 'xy'] }))).toEqual({
			dim: true,
			color: true,
			temp: true
		});
		expect(lightCaps(st({ supported_color_modes: ['onoff'] }))).toEqual({
			dim: false,
			color: false,
			temp: false
		});
		expect(lightCaps(st({ brightness: 120 }))).toEqual({ dim: true, color: false, temp: false });
	});
});
