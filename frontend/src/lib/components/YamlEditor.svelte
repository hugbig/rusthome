<script lang="ts">
	// Éditeur YAML (CodeMirror 6). Le contenu est la seule source : le parent
	// lit `value` via bind:value et l'envoie tel quel au serveur, qui valide.
	import { onDestroy, onMount } from 'svelte';
	import { basicSetup, EditorView } from 'codemirror';
	import { yaml } from '@codemirror/lang-yaml';
	import { oneDark } from '@codemirror/theme-one-dark';
	import { EditorState } from '@codemirror/state';

	let { value = $bindable(''), readonly = false }: { value?: string; readonly?: boolean } =
		$props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;

	onMount(() => {
		const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc: value,
				extensions: [
					basicSetup,
					yaml(),
					EditorState.tabSize.of(2),
					EditorState.readOnly.of(readonly),
					EditorView.updateListener.of((u) => {
						if (u.docChanged) value = u.state.doc.toString();
					}),
					...(dark ? [oneDark] : [])
				]
			})
		});
	});

	// Valeur remplacée de l'extérieur (rechargement) : on synchronise l'éditeur.
	$effect(() => {
		const v = value;
		if (view && v !== view.state.doc.toString()) {
			view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v } });
		}
	});

	onDestroy(() => view?.destroy());
</script>

<div class="editor" bind:this={host}></div>

<style>
	.editor {
		border: 1px solid var(--border);
		border-radius: 8px;
		overflow: hidden;
		font-size: 0.9rem;
	}
	.editor :global(.cm-editor) {
		max-height: 70vh;
	}
	.editor :global(.cm-scroller) {
		font-family: ui-monospace, 'Cascadia Code', Menlo, monospace;
	}
</style>
