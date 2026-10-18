import { describe, expect, it } from 'vitest';
import { devicesOf, eventLabel, eventsOf, groupsOf, lqiLevel, permitJoinLeft } from './zigbee';

const snapshot = {
	info: { permit_join: true, permit_join_end: 1_000_120_000 },
	devices: [
		{ ieee_address: '0x00', friendly_name: 'Coordinator', type: 'Coordinator' },
		{
			ieee_address: '0x02',
			friendly_name: 'Salon',
			type: 'EndDevice',
			power_source: 'Battery',
			interview_completed: true,
			supported: true,
			definition: { model: 'WSDCGQ11LM', vendor: 'Aqara', description: 'Capteur' }
		},
		{ ieee_address: '0x01', friendly_name: 'Bureau', type: 'Router', interview_completed: false }
	],
	groups: [
		{ id: 3, friendly_name: 'Salon tout', members: [{ ieee_address: '0x02', endpoint: 1 }] }
	],
	events: [
		{
			type: 'device_interview',
			data: { friendly_name: 'Bureau', status: 'successful' },
			received: 5
		},
		{ type: 'device_joined', data: { ieee_address: '0x09' }, received: 4 }
	],
	payloads: { Salon: { linkquality: 87, battery: 64, update: { state: 'available' } } }
};

describe('Zigbee', () => {
	it('lit les appareils avec leur dernier état, sans le coordinateur', () => {
		const ds = devicesOf(snapshot);
		expect(ds.map((d) => d.name)).toEqual(['Bureau', 'Salon']);
		expect(ds[1]).toMatchObject({
			ieee: '0x02',
			model: 'WSDCGQ11LM',
			vendor: 'Aqara',
			lqi: 87,
			battery: 64,
			ota: 'available',
			interviewed: true
		});
		expect(ds[0]!.interviewed).toBe(false);
		expect(devicesOf(null)).toEqual([]);
	});

	it('lit groupes et événements', () => {
		expect(groupsOf(snapshot)).toEqual([
			{ id: 3, name: 'Salon tout', members: [{ ieee: '0x02', endpoint: 1 }] }
		]);
		const ev = eventsOf(snapshot);
		expect(eventLabel(ev[0]!)).toBe('Bureau : identification réussie');
		expect(eventLabel(ev[1]!)).toBe('0x09 a rejoint le réseau');
	});

	it('calcule le temps d’appairage restant', () => {
		expect(permitJoinLeft(snapshot, 1_000_000_000)).toBe(120);
		expect(permitJoinLeft({ info: { permit_join: false } })).toBe(0);
		expect(permitJoinLeft({ info: { permit_join: true, permit_join_timeout: 60 } })).toBe(60);
		expect([lqiLevel(150), lqiLevel(50), lqiLevel(10), lqiLevel(undefined)]).toEqual([
			'good',
			'warning',
			'critical',
			undefined
		]);
	});
});
