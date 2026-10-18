import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { live } from '../stores/live.svelte';
import type { View } from './model';
import ViewLayout from './ViewLayout.svelte';

const view: View = {
	type: 'sections',
	sections: [
		{
			cards: [
				{ type: 'heading', heading: 'Alarme' },
				{ type: 'tile', entity: 'light.salon', name: 'Salon' },
				{ type: 'custom:inconnue' }
			]
		}
	]
};

describe('ViewLayout', () => {
	it('rend les cartes, y compris non supportées', () => {
		live.states.set('light.salon', {
			entity_id: 'light.salon',
			state: 'on',
			attributes: {},
			last_changed: '2026-10-17T10:00:00Z',
			last_updated: '2026-10-17T10:00:00Z',
			context: { id: 'c' }
		});
		render(ViewLayout, { view, viewIndex: 0 });
		expect(screen.getByText('Alarme')).toBeInTheDocument();
		expect(screen.getByText('Allumé')).toBeInTheDocument();
		expect(screen.getByText('Carte non supportée')).toBeInTheDocument();
		expect(screen.queryByText('Modifier')).toBeNull();
	});

	it('en édition : Modifier et menu ⋮ par carte, ajout de carte et de section', async () => {
		const onop = vi.fn();
		render(ViewLayout, { view, viewIndex: 0, editing: true, onop });
		const edit = screen.getAllByText('Modifier');
		expect(edit).toHaveLength(3);
		await fireEvent.click(edit[1]!);
		expect(onop).toHaveBeenLastCalledWith({ kind: 'edit', loc: { view: 0, section: 0, index: 1 } });

		await fireEvent.click(screen.getAllByLabelText('Options de la carte')[0]!);
		expect(screen.getByRole('menuitem', { name: 'Monter' })).toBeDisabled();
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Dupliquer' }));
		expect(onop).toHaveBeenLastCalledWith({
			kind: 'duplicate',
			loc: { view: 0, section: 0, index: 0 }
		});

		await fireEvent.click(screen.getByText(/Ajouter une carte/));
		expect(onop).toHaveBeenLastCalledWith({ kind: 'add', where: { view: 0, section: 0 } });
		await fireEvent.click(screen.getByText(/Créer une section/));
		expect(onop).toHaveBeenLastCalledWith({ kind: 'addSection' });
	});
});
