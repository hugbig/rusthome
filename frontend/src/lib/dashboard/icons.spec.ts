import { describe, expect, it } from 'vitest';
import { searchIcons } from './icons';

const names = ['sofa', 'sofa-outline', 'lightbulb', 'lightbulb-outline', 'ceiling-light', 'lamp'];

describe('searchIcons', () => {
	it('trouve par mots, préfixe puis longueur d’abord', () => {
		expect(searchIcons(names, 'light')).toEqual([
			'mdi:lightbulb',
			'mdi:lightbulb-outline',
			'mdi:ceiling-light'
		]);
		expect(searchIcons(names, 'mdi:sofa')).toEqual(['mdi:sofa', 'mdi:sofa-outline']);
		expect(searchIcons(names, 'outline bulb')).toEqual(['mdi:lightbulb-outline']);
	});

	it('ne renvoie rien sans recherche et respecte la limite', () => {
		expect(searchIcons(names, '  ')).toEqual([]);
		expect(searchIcons(names, 'l', 2)).toHaveLength(2);
	});
});
