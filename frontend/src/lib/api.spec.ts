import { describe, expect, it } from 'vitest';
import { callService, explainError } from './api';

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
