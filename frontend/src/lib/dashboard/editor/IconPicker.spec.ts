import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { iconSet } from '../icon-set.svelte';
import IconPicker from './IconPicker.svelte';

describe('IconPicker', () => {
	it('cherche une icône, la choisit, puis revient à celle par défaut', async () => {
		iconSet.paths = { sofa: 'M0 0', 'sofa-outline': 'M1 1', lamp: 'M2 2' };
		const onchange = vi.fn();
		const { rerender } = render(IconPicker, { entityId: 'light.salon', onchange });
		const input = screen.getByRole('combobox', { name: 'Icône' });

		await fireEvent.focus(input);
		await fireEvent.input(input, { target: { value: 'sofa' } });
		const options = screen.getAllByRole('option');
		expect(options.map((o) => o.textContent?.trim())).toEqual(['mdi:sofa', 'mdi:sofa-outline']);

		await fireEvent.click(options[1]!.querySelector('button')!);
		expect(onchange).toHaveBeenLastCalledWith('mdi:sofa-outline');

		await rerender({ value: 'mdi:sofa-outline', entityId: 'light.salon', onchange });
		await fireEvent.click(screen.getByRole('button', { name: 'Revenir à l’icône par défaut' }));
		expect(onchange).toHaveBeenLastCalledWith(undefined);
	});
});
