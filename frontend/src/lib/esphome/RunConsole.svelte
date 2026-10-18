<script lang="ts">
	// Opération ESPHome en direct (validation, compilation, installation,
	// logs) : la sortie du Builder défile avec ses couleurs. Fermer arrête
	// l'opération.
	import { onDestroy, onMount } from 'svelte';
	import Dialog from '../components/Dialog.svelte';
	import { ACTION_LABELS, ansiSegments, runUrl, type EspAction, type RunMessage } from '../esphome';

	interface Props {
		configuration: string;
		title: string;
		action: EspAction;
		/** Adresse affichée (fixée par le serveur). */
		target?: string;
		onclose: () => void;
		/** Opération terminée (pour rafraîchir la liste). */
		ondone?: (ok: boolean) => void;
	}
	let { configuration, title, action, target, onclose, ondone }: Props = $props();

	const MAX_LINES = 5000;
	let lines = $state<{ id: number; text: string }[]>([]);
	let step = $state('Connexion…');
	let status = $state<'running' | 'ok' | 'failed'>('running');
	let error = $state<string | null>(null);
	let box = $state<HTMLPreElement>();
	let follow = $state(true);
	let ws: WebSocket | undefined;
	let seq = 0;

	onMount(() => {
		ws = new WebSocket(runUrl(location, configuration, action));
		ws.onmessage = (m) => {
			const msg = JSON.parse(String(m.data)) as RunMessage;
			if (msg.type === 'step') step = msg.text;
			else if (msg.type === 'line') {
				lines.push({ id: seq++, text: msg.text });
				if (lines.length > MAX_LINES) lines.splice(0, lines.length - MAX_LINES);
			} else {
				status = msg.ok ? 'ok' : 'failed';
				error = msg.error ?? null;
				ondone?.(msg.ok);
			}
		};
		ws.onclose = () => {
			if (status === 'running') {
				status = 'failed';
				error ??= 'Connexion interrompue.';
			}
		};
	});
	onDestroy(() => ws?.close());

	// Défilement automatique, sauf si l'utilisateur remonte lire.
	$effect(() => {
		void lines.length;
		if (follow && box) box.scrollTop = box.scrollHeight;
	});
	function scrolled() {
		if (box) follow = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
	}

	const statusLabel = $derived(
		status === 'running'
			? step
			: status === 'ok'
				? `${ACTION_LABELS[action]} réussie`
				: `${ACTION_LABELS[action]} en échec`
	);
</script>

<Dialog title="{ACTION_LABELS[action]} · {title}" {onclose} width={980}>
	<p class="status {status}" role="status">
		<span class="dot" aria-hidden="true"></span>{statusLabel}
		{#if target}<small>vers {target}</small>{/if}
	</p>
	{#if error}<p class="err">{error}</p>{/if}
	<!-- Zone défilante : focalisable pour la lire au clavier (WCAG 2.1.1). -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<pre
		role="log"
		bind:this={box}
		onscroll={scrolled}
		aria-label="Sortie"
		tabindex="0">{#each lines as l (l.id)}<span class="ln"
				>{#each ansiSegments(l.text) as s, i (i)}<span class={s.color} class:b={s.bold}
						>{s.text}</span
					>{/each}</span
			>{/each}</pre>
	{#snippet footer()}
		<button class="dbtn" onclick={onclose}
			>{status === 'running'
				? action === 'logs'
					? 'Arrêter les logs'
					: 'Arrêter et fermer'
				: 'Fermer'}</button
		>
	{/snippet}
</Dialog>

<style>
	.status {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		font-weight: 600;
	}
	.status small {
		color: var(--text-muted);
		font-weight: 400;
	}
	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--text-muted);
	}
	.running .dot {
		background: var(--primary);
		animation: pulse 1.2s ease-in-out infinite;
	}
	.ok .dot {
		background: var(--good);
	}
	.failed .dot {
		background: var(--critical);
	}
	@keyframes pulse {
		50% {
			opacity: 0.3;
		}
	}
	.err {
		margin: 0;
		color: var(--critical);
	}
	pre {
		margin: 0;
		height: min(60vh, 560px);
		overflow: auto;
		padding: 10px 12px;
		border-radius: 12px;
		background: #0d1117;
		color: #c9d1d9;
		font-size: 0.78rem;
		line-height: 1.45;
	}
	.ln {
		display: block;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.b {
		font-weight: 700;
	}
	.red {
		color: #ff7b72;
	}
	.green {
		color: #7ee787;
	}
	.yellow {
		color: #e3b341;
	}
	.blue {
		color: #79c0ff;
	}
	.magenta {
		color: #d2a8ff;
	}
	.cyan {
		color: #56d4dd;
	}
	.gray {
		color: #8b949e;
	}
	.white {
		color: #f0f6fc;
	}
</style>
