import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import ControlSlider from './ControlSlider.svelte';

describe('ControlSlider', () => {
	it('expose sa valeur et se règle au clavier dans les bornes', async () => {
		const onchange = vi.fn();
		render(ControlSlider, { value: 56, label: 'Luminosité', onchange });
		const s = screen.getByRole('slider', { name: 'Luminosité' });
		expect(s.getAttribute('aria-valuenow')).toBe('56');
		expect(s.getAttribute('aria-valuetext')).toBe('56 %');

		await fireEvent.keyDown(s, { key: 'ArrowRight' });
		await fireEvent.keyDown(s, { key: 'PageDown' });
		await fireEvent.keyDown(s, { key: 'End' });
		await fireEvent.keyDown(s, { key: 'a' });
		expect(onchange.mock.calls).toEqual([[57], [46], [100]]);
	});

	it('n’envoie rien quand la valeur ne change pas', async () => {
		const onchange = vi.fn();
		render(ControlSlider, { value: 100, label: 'Pleine', onchange });
		await fireEvent.keyDown(screen.getByRole('slider', { name: 'Pleine' }), { key: 'ArrowUp' });
		expect(onchange).not.toHaveBeenCalled();
	});
});
