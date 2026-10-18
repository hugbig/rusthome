import { describe, expect, it } from 'vitest';
import {
	ansiSegments,
	runUrl,
	sortDevices,
	stateLabel,
	validName,
	type EspDevice
} from './esphome';

describe('ESPHome', () => {
	it('colore la sortie, codes bruts ou échappés', () => {
		expect(ansiSegments('\\033[32mINFO ok\\033[0m fin')).toEqual([
			{ text: 'INFO ok', color: 'green' },
			{ text: ' fin' }
		]);
		expect(ansiSegments('\x1b[1;31mFailed\x1b[0m')).toEqual([
			{ text: 'Failed', color: 'red', bold: true }
		]);
		expect(ansiSegments('texte simple')).toEqual([{ text: 'texte simple' }]);
		expect(ansiSegments('')).toEqual([]);
	});

	it('construit l’adresse d’une opération', () => {
		expect(runUrl({ protocol: 'https:', host: 'maison' }, 'test-nfc.yaml', 'install')).toBe(
			'wss://maison/api/v1/esphome/configs/test-nfc.yaml/run?action=install'
		);
		expect(runUrl({ protocol: 'http:', host: 'h:1' }, 'a.yaml', 'logs')).toBe(
			'ws://h:1/api/v1/esphome/configs/a.yaml/run?action=logs'
		);
	});

	it('vérifie les noms d’hôte', () => {
		expect([validName('salon-ir'), validName('Salon'), validName('-x')]).toEqual([
			true,
			false,
			false
		]);
	});

	it('libellés et tri', () => {
		expect(stateLabel('online')).toBe('En ligne');
		const d = (configuration: string, friendly_name: string | null) =>
			({ configuration, friendly_name, name: null }) as EspDevice;
		expect(
			sortDevices([d('b.yaml', 'Zèbre'), d('a.yaml', null), d('c.yaml', 'Éclairage')]).map(
				(x) => x.configuration
			)
		).toEqual(['a.yaml', 'c.yaml', 'b.yaml']);
	});
});
