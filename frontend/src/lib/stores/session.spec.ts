import { describe, expect, it } from 'vitest';
import { loginUrl } from './session.svelte';

describe('loginUrl', () => {
	it('encode la page de retour', () => {
		expect(loginUrl('/entites?etat=indisponible')).toBe(
			'/api/v1/auth/login?next=%2Fentites%3Fetat%3Dindisponible'
		);
	});
});
