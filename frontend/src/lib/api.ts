/** Résultat d'un appel de service : le message d'erreur du serveur est conservé. */
export type CallResult = { ok: true; data: unknown } | { ok: false; status: number; error: string };

/** `POST /api/v1/services/{domain}/{service}`. */
export async function callService(
	domain: string,
	service: string,
	data: Record<string, unknown>,
	fetcher: typeof fetch = fetch
): Promise<CallResult> {
	try {
		const r = await fetcher(`/api/v1/services/${domain}/${service}`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(data)
		});
		const body: unknown = await r.json().catch(() => null);
		if (r.ok) return { ok: true, data: body };
		const error =
			body && typeof body === 'object' && 'error' in body ? String(body.error) : `HTTP ${r.status}`;
		return { ok: false, status: r.status, error };
	} catch (e) {
		return { ok: false, status: 0, error: e instanceof Error ? e.message : String(e) };
	}
}

/** Message français pour les refus connus du serveur. */
export function explainError(r: Extract<CallResult, { ok: false }>): string {
	if (r.error.includes('read-only (RUSTHOME_MQTT_READ_ONLY'))
		return 'Commande refusée : MQTT est en lecture seule pendant le développement (rien n’a été envoyé).';
	if (r.error.includes('driven by Home Assistant'))
		return 'Cette entité est encore pilotée par Home Assistant : RustHome ne fait que la lire.';
	if (r.status === 403) return 'Votre compte ne peut pas envoyer de commandes.';
	return `Échec de la commande : ${r.error}`;
}
