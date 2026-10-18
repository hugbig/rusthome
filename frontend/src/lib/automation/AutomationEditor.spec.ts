import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import AutomationEditor from './AutomationEditor.svelte';

const doc = {
	alias: 'Lumière entrée',
	mode: 'single',
	triggers: [{ trigger: 'state', entity_id: ['binary_sensor.porte'], to: 'on' }],
	conditions: [],
	actions: [{ action: 'light.turn_on', target: { entity_id: ['light.entree'] } }]
};

describe('AutomationEditor', () => {
	afterEach(cleanup);

	it('affiche les blocs résumés et ajoute une action au format HA', async () => {
		const onchange = vi.fn();
		render(AutomationEditor, { doc, onchange });
		expect(screen.getByText('Quand binary_sensor.porte change à on')).toBeInTheDocument();
		expect(screen.getByText('light.turn_on sur light.entree')).toBeInTheDocument();

		await fireEvent.click(screen.getByText('Ajouter une action'));
		await fireEvent.click(screen.getByRole('button', { name: 'Attendre une durée' }));
		const next = onchange.mock.lastCall![0] as typeof doc;
		expect(next.actions).toHaveLength(2);
		expect(next.actions[1]).toEqual({ delay: '00:00:30' });
		expect(next.triggers).toBe(doc.triggers);
	});

	it('modifie un champ d’un bloc (déplié par défaut)', async () => {
		const onchange = vi.fn();
		render(AutomationEditor, { doc, onchange });
		// deux champs « tout état » (De, À) : le second est « À »
		const fields = screen.getAllByPlaceholderText('tout état') as HTMLInputElement[];
		expect(fields[1]!.value).toBe('on');
		await fireEvent.input(fields[1]!, { target: { value: 'off' } });
		const next = onchange.mock.lastCall![0] as typeof doc;
		expect(next.triggers[0]).toMatchObject({ trigger: 'state', to: 'off' });
	});

	it('accepte les anciennes clés au singulier', async () => {
		const onchange = vi.fn();
		render(AutomationEditor, {
			doc: { alias: 'x', trigger: { platform: 'time', at: '07:00:00' }, action: [] },
			onchange
		});
		expect(screen.getByText('À 07:00:00')).toBeInTheDocument();
	});
});
