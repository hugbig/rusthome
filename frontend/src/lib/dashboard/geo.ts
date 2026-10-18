// Carte sans bibliothèque : tuiles OpenStreetMap positionnées à la main
// (projection Web Mercator, tuiles de 256 px).

export interface LatLon {
	lat: number;
	lon: number;
}

/** Coordonnées en pixels « monde » au niveau de zoom `z`. */
export function project({ lat, lon }: LatLon, z: number): { x: number; y: number } {
	const size = 256 * 2 ** z;
	const s = Math.sin((lat * Math.PI) / 180);
	return {
		x: ((lon + 180) / 360) * size,
		y: (0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI)) * size
	};
}

export interface Tile {
	x: number;
	y: number;
	left: number;
	top: number;
}

/** Tuiles qui couvrent une fenêtre `w`×`h` centrée sur `center`. */
export function tilesFor(center: LatLon, z: number, w: number, h: number): Tile[] {
	const c = project(center, z);
	const x0 = c.x - w / 2;
	const y0 = c.y - h / 2;
	const out: Tile[] = [];
	const max = 2 ** z;
	for (let ty = Math.floor(y0 / 256); ty <= Math.floor((y0 + h) / 256); ty++) {
		if (ty < 0 || ty >= max) continue;
		for (let tx = Math.floor(x0 / 256); tx <= Math.floor((x0 + w) / 256); tx++) {
			out.push({ x: ((tx % max) + max) % max, y: ty, left: tx * 256 - x0, top: ty * 256 - y0 });
		}
	}
	return out;
}

/** Position d'un point dans la fenêtre centrée sur `center`. */
export function pointIn(p: LatLon, center: LatLon, z: number, w: number, h: number) {
	const a = project(p, z);
	const c = project(center, z);
	return { left: a.x - c.x + w / 2, top: a.y - c.y + h / 2 };
}

/** Plus grand zoom (≤ 16) qui fait tenir tous les points dans la fenêtre. */
export function fitZoom(points: LatLon[], w: number, h: number): number {
	for (let z = 16; z > 2; z--) {
		const px = points.map((p) => project(p, z));
		const dx = Math.max(...px.map((p) => p.x)) - Math.min(...px.map((p) => p.x));
		const dy = Math.max(...px.map((p) => p.y)) - Math.min(...px.map((p) => p.y));
		if (dx <= w - 60 && dy <= h - 60) return z;
	}
	return 2;
}

export function centerOf(points: LatLon[]): LatLon {
	const n = Math.max(1, points.length);
	return {
		lat: points.reduce((a, p) => a + p.lat, 0) / n,
		lon: points.reduce((a, p) => a + p.lon, 0) / n
	};
}
