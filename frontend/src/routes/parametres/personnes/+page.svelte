<script lang="ts">
	import { resolve } from '$app/paths';
	import TileCard from '#lib/dashboard/cards/TileCard.svelte';
	import { live } from '#lib/stores/live.svelte.ts';
	import { ROLE_LABELS, session } from '#lib/stores/session.svelte.ts';

	const persons = $derived([...live.states.keys()].filter((id) => id.startsWith('person.')).sort());
	const ROLES = [
		{ role: 'admin', who: 'admin', what: 'Tout, y compris paramètres et intégrations' },
		{
			role: 'resident',
			who: 'hugo, sokhna',
			what: 'Tableaux de bord, commandes, alarme, automatisations et tableaux non verrouillés'
		},
		{
			role: 'display',
			who: 'tablette murale',
			what: 'Lecture et commandes limitées, sans désarmement'
		}
	] as const;
</script>

<p class="crumbs"><a href={resolve('/parametres')}>Paramètres</a></p>
<h1>Personnes et comptes</h1>

<h2>Personnes</h2>
<div class="grid">
	{#each persons as id (id)}
		<TileCard card={{ type: 'tile', entity: id }} />
	{:else}
		<p class="muted">Aucune personne suivie.</p>
	{/each}
</div>

<h2>Comptes et rôles</h2>
<section class="card">
	<p class="note">
		Les comptes viennent d’Authelia (connexion unique) ; RustHome n’attribue que le rôle, via
		<code>RUSTHOME_ROLES</code>. Connecté : <strong>{session.me?.name}</strong>
		({session.me ? ROLE_LABELS[session.me.role] : '—'}).
	</p>
	<table>
		<thead><tr><th>Rôle</th><th>Comptes</th><th>Droits</th></tr></thead>
		<tbody>
			{#each ROLES as r (r.role)}
				<tr><td>{ROLE_LABELS[r.role]}</td><td>{r.who}</td><td class="muted">{r.what}</td></tr>
			{/each}
		</tbody>
	</table>
</section>

<style>
	.crumbs {
		margin: 0 0 8px;
		font-size: 0.9rem;
	}
	h2 {
		margin-top: 20px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 8px;
		margin-bottom: 16px;
	}
	.note,
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th {
		text-align: left;
		color: var(--text-muted);
		font-size: 0.8rem;
		padding: 6px;
	}
	td {
		padding: 8px 6px;
		border-top: 1px solid var(--border);
	}
</style>
