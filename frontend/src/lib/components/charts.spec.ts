import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import LineChart from './LineChart.svelte';
import StateTimeline from './StateTimeline.svelte';

const from = Date.parse('2026-10-17T00:00:00Z');
const to = Date.parse('2026-10-17T12:00:00Z');

describe('LineChart', () => {
	it('trace une courbe et son point final, avec un libellé accessible', () => {
		const { container } = render(LineChart, {
			points: [
				{ t: from + 3600_000, v: 19.5 },
				{ t: from + 7200_000, v: 20.5 }
			],
			from,
			to,
			unit: '°C',
			label: 'Historique de Température'
		});
		expect(screen.getByRole('img', { name: 'Historique de Température' })).toBeInTheDocument();
		expect(container.querySelectorAll('path.line')).toHaveLength(1);
		expect(container.querySelector('circle.end')).not.toBeNull();
	});

	it('coupe la courbe sur une indisponibilité', () => {
		const { container } = render(LineChart, {
			points: [{ t: from + 1000, v: 1 }, null, { t: from + 5000, v: 2 }],
			from,
			to,
			label: 'x'
		});
		expect(container.querySelectorAll('path.line')).toHaveLength(2);
	});

	it('affiche un message sans valeur numérique', () => {
		render(LineChart, { points: [null], from, to, label: 'x' });
		expect(screen.getByText(/Pas encore de valeur/)).toBeInTheDocument();
	});
});

describe('StateTimeline', () => {
	it('dessine un segment par état et une légende étiquetée', () => {
		const { container } = render(StateTimeline, {
			segments: [
				{ state: 'disarmed', from, to: from + 3600_000 },
				{ state: 'armed_away', from: from + 3600_000, to }
			],
			from,
			to,
			label: "États de l'alarme"
		});
		expect(container.querySelectorAll('svg[role="img"] > rect')).toHaveLength(2);
		// Identité jamais portée par la seule couleur : chaque état est nommé dans la légende.
		expect(screen.getByText('disarmed')).toBeInTheDocument();
		expect(screen.getByText('armed_away')).toBeInTheDocument();
		expect(screen.getByText('11 h 00')).toBeInTheDocument();
	});
});
