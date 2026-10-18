import { describe, expect, it } from 'vitest';
import {
	duration,
	frigateFile,
	haThumb,
	hourRange,
	kindOf,
	labelFr,
	prettyFileName,
	scoreOf
} from './media';

describe('navigateur multimédia', () => {
	it('traduit et mesure les événements Frigate', () => {
		expect(labelFr('person')).toBe('Personne');
		expect(labelFr('license_plate')).toBe('Plaque');
		expect(labelFr('raccoon_baby')).toBe('raccoon baby');
		const e = {
			id: 'x',
			camera: 'salon',
			start_time: 100,
			end_time: 285,
			data: { top_score: 0.874 }
		};
		expect(scoreOf(e)).toBe(87);
		expect(duration(e)).toBe('3 min 05 s');
		expect(duration({ ...e, end_time: 142 })).toBe('42 s');
		expect(duration({ ...e, end_time: null })).toBeUndefined();
	});

	it('calcule une heure d’enregistrement locale', () => {
		const [s, e] = hourRange('2026-10-17', 14);
		expect(new Date(s * 1000).getHours()).toBe(14);
		expect(e - s).toBe(3600);
		expect(frigateFile.recording('salon', s, e)).toBe(
			`/api/v1/media/frigate/file/recording?camera=salon&start=${s}&end=${e}`
		);
		expect(frigateFile.recordingHls('salon', s, e)).toBe(
			`/api/v1/media/frigate/vod/recording/salon/${s}/${e}/index.m3u8`
		);
		expect(frigateFile.clipHls('1.2-ab')).toBe('/api/v1/media/frigate/vod/event/1.2-ab/index.m3u8');
	});

	it('reconnaît la nature des médias de HA', () => {
		expect(kindOf({ can_expand: true, media_class: 'directory' })).toBe('folder');
		expect(kindOf({ media_class: 'video', media_content_type: 'video/mp4' })).toBe('video');
		expect(kindOf({ media_class: 'music', media_content_type: 'audio/mpeg' })).toBe('audio');
		expect(kindOf({ media_class: 'image', media_content_type: 'image/jpeg' })).toBe('image');
		expect(haThumb('/api/frigate/thumb.jpg')).toBe(
			'/api/v1/media/ha/file?path=%2Fapi%2Ffrigate%2Fthumb.jpg'
		);
		expect(haThumb('https://brands.home-assistant.io/x.png')).toBeUndefined();
	});

	it('rend lisibles les noms de fichiers datés', () => {
		expect(prettyFileName('20261001-103837-salon.mp4')).toMatch(/1 oct\..*10:38 · salon$/);
		expect(prettyFileName('alarm.mp3')).toBe('alarm.mp3');
	});
});
