import { describe, expect, it } from 'vitest';
import { callService, explainError, request } from './api';

const fakeFetch = (status: number, body: unknown) =>
	(async () => new Response(JSON.stringify(body), { status })) as unknown as typeof fetch;

describe('callService', () => {
	it('renvoie les données en cas de succès', async () => {
		const r = await callService(
			'switch',
			'turn_on',
			{ entity_id: 'switch.x' },
			fakeFetch(200, [{ topic: 't' }])
		);
		expect(r).toEqual({ ok: true, data: [{ topic: 't' }] });
	});

	it("conserve et explique l'erreur du serveur", async () => {
		const r = await callService(
			'switch',
			'turn_on',
			{ entity_id: 'switch.x' },
			fakeFetch(409, {
				error: 'MQTT is read-only (RUSTHOME_MQTT_READ_ONLY=true): command not sent'
			})
		);
		expect(r.ok).toBe(false);
		if (!r.ok) {
			expect(r.status).toBe(409);
			expect(explainError(r)).toContain('lecture seule');
		}
	});

	it('refuse un nom de service qui viserait une autre route', async () => {
		let called = false;
		const f = (async () => {
			called = true;
			return new Response('{}');
		}) as unknown as typeof fetch;
		const r = await callService('x', 'y/../../zigbee/devices/0x1/remove', {}, f);
		expect(r.ok).toBe(false);
		expect(called).toBe(false);
	});

	it('explique une entité pilotée par Home Assistant', () => {
		expect(
			explainError({
				ok: false,
				status: 409,
				error: 'light.salon is still driven by Home Assistant (read-only mirror)'
			})
		).toContain('Home Assistant');
	});
});

describe('request', () => {
	it('envoie le corps en JSON et renvoie les données', async () => {
		let seen: RequestInit | undefined;
		const f = (async (_url: string, init?: RequestInit) => {
			seen = init;
			return new Response(JSON.stringify({ id: 'a1' }), { status: 200 });
		}) as unknown as typeof fetch;
		const r = await request<{ id: string }>('PUT', 'automations/a1', { yaml: 'x' }, f);
		expect(r).toEqual({ ok: true, data: { id: 'a1' } });
		expect(seen?.method).toBe('PUT');
		expect(seen?.body).toBe('{"yaml":"x"}');
	});

	it("remonte le message d'erreur du serveur", async () => {
		const r = await request(
			'POST',
			'automations/x/enable',
			undefined,
			fakeFetch(409, { error: 'has errors' })
		);
		expect(r).toEqual({ ok: false, status: 409, error: 'has errors' });
	});
});
