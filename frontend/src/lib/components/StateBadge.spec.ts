import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import StateBadge from './StateBadge.svelte';
import type { EntityState } from '../types';

const entity = (state: string, attributes: Record<string, unknown> = {}): EntityState => ({
	entity_id: 'sensor.x',
	state,
	attributes,
	last_changed: '2026-10-17T10:00:00Z',
	last_updated: '2026-10-17T10:00:00Z',
	context: { id: 'c' }
});

describe('StateBadge', () => {
	it("affiche l'unité", () => {
		render(StateBadge, { entity: entity('20.6', { unit_of_measurement: '°C' }) });
		expect(screen.getByText(/20\.6\s°C/)).toBeInTheDocument();
	});

	it('met en avant les états actifs et atténue les indisponibles', () => {
		const { container } = render(StateBadge, { entity: entity('on') });
		expect(container.querySelector('.active')).not.toBeNull();
		const other = render(StateBadge, { entity: entity('unavailable') });
		expect(other.container.querySelector('.muted')).not.toBeNull();
	});
});
