// Commande des lumières (fiche) : roue chromatique et modes de couleur.
import type { EntityState } from '../types';

/** Point de la roue (centre = blanc, bord = saturé ; teinte 0° à droite, sens horaire). */
export function wheelToHs(dx: number, dy: number, radius: number): [number, number] {
	const h = ((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360;
	const s = Math.min(1, Math.hypot(dx, dy) / Math.max(1, radius)) * 100;
	return [Math.round(h), Math.round(s)];
}

export function hsToWheel(h: number, s: number, radius: number): [number, number] {
	const a = (h * Math.PI) / 180;
	const r = (Math.min(100, Math.max(0, s)) / 100) * radius;
	return [r * Math.cos(a), r * Math.sin(a)];
}

const COLOR_MODES = new Set(['hs', 'xy', 'rgb', 'rgbw', 'rgbww']);

/** Ce que la lumière sait faire : variation, couleur, blanc réglable. */
export function lightCaps(s: EntityState | undefined): {
	dim: boolean;
	color: boolean;
	temp: boolean;
} {
	const a = s?.attributes ?? {};
	const modes = Array.isArray(a.supported_color_modes) ? (a.supported_color_modes as string[]) : [];
	if (modes.length) {
		const color = modes.some((m) => COLOR_MODES.has(m));
		const temp = modes.includes('color_temp');
		return {
			dim: color || temp || modes.includes('brightness') || modes.includes('white'),
			color,
			temp
		};
	}
	// lumière sans liste de modes : on déduit des attributs présents
	return {
		dim: typeof a.brightness === 'number',
		color: Array.isArray(a.rgb_color) || Array.isArray(a.hs_color),
		temp: typeof a.color_temp_kelvin === 'number'
	};
}
