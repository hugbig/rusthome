// Icônes Material Design : seules celles des dashboards et des domaines sont
// embarquées (import nommé, le reste de @mdi/js est éliminé au build).
import {
	mdiAccount,
	mdiAlarmLight,
	mdiAlarmLightOutline,
	mdiArrowDown,
	mdiArrowUp,
	mdiBatteryAlert,
	mdiCalendar,
	mdiCctv,
	mdiCheckboxBlankCircleOutline,
	mdiClockDigital,
	mdiClose,
	mdiCounter,
	mdiDelete,
	mdiEye,
	mdiFan,
	mdiFlash,
	mdiFormatListBulleted,
	mdiGestureTapButton,
	mdiHelpCircleOutline,
	mdiHistory,
	mdiHome,
	mdiHomeAnalytics,
	mdiHomeGroup,
	mdiLan,
	mdiLightbulb,
	mdiLightbulbOn90,
	mdiLock,
	mdiMapMarker,
	mdiMinus,
	mdiMotionSensor,
	mdiMultimedia,
	mdiNas,
	mdiPackageUp,
	mdiPalette,
	mdiPause,
	mdiPencil,
	mdiPlay,
	mdiPlus,
	mdiRayVertex,
	mdiRobot,
	mdiScriptText,
	mdiSettingsHelper,
	mdiShieldHome,
	mdiShieldLock,
	mdiSkipNext,
	mdiSkipPrevious,
	mdiSpeaker,
	mdiTabletDashboard,
	mdiThermometer,
	mdiThermostat,
	mdiToggleSwitch,
	mdiToggleSwitchOutline,
	mdiViewDashboard,
	mdiVolumeHigh,
	mdiVolumeMinus,
	mdiVolumePlus,
	mdiWeatherCloudy,
	mdiWeatherPartlyCloudy,
	mdiWindowShutter
} from '@mdi/js';
import { domainOf } from '../format';
import { iconSet } from './icon-set.svelte';

const NAMED: Record<string, string> = {
	'mdi:account': mdiAccount,
	'mdi:alarm-light': mdiAlarmLight,
	'mdi:alarm-light-outline': mdiAlarmLightOutline,
	'mdi:battery-alert': mdiBatteryAlert,
	'mdi:calendar': mdiCalendar,
	'mdi:cctv': mdiCctv,
	'mdi:clock-digital': mdiClockDigital,
	'mdi:fan': mdiFan,
	'mdi:flash': mdiFlash,
	'mdi:gesture-tap-button': mdiGestureTapButton,
	'mdi:history': mdiHistory,
	'mdi:home': mdiHome,
	'mdi:home-analytics': mdiHomeAnalytics,
	'mdi:home-group': mdiHomeGroup,
	'mdi:lan': mdiLan,
	'mdi:lightbulb': mdiLightbulb,
	'mdi:lightbulb-on-90': mdiLightbulbOn90,
	'mdi:motion-sensor': mdiMotionSensor,
	'mdi:multimedia': mdiMultimedia,
	'mdi:nas': mdiNas,
	'mdi:robot': mdiRobot,
	'mdi:settings-helper': mdiSettingsHelper,
	'mdi:shield-home': mdiShieldHome,
	'mdi:shield-lock': mdiShieldLock,
	'mdi:speaker': mdiSpeaker,
	'mdi:tablet-dashboard': mdiTabletDashboard,
	'mdi:thermometer': mdiThermometer,
	'mdi:view-dashboard': mdiViewDashboard,
	'mdi:weather-cloudy': mdiWeatherCloudy
};

const BY_DOMAIN: Record<string, string> = {
	alarm_control_panel: mdiShieldHome,
	automation: mdiRobot,
	binary_sensor: mdiCheckboxBlankCircleOutline,
	button: mdiGestureTapButton,
	calendar: mdiCalendar,
	camera: mdiCctv,
	climate: mdiThermostat,
	counter: mdiCounter,
	cover: mdiWindowShutter,
	device_tracker: mdiMapMarker,
	fan: mdiFan,
	input_boolean: mdiToggleSwitchOutline,
	input_select: mdiFormatListBulleted,
	light: mdiLightbulb,
	lock: mdiLock,
	media_player: mdiSpeaker,
	number: mdiRayVertex,
	person: mdiAccount,
	scene: mdiPalette,
	script: mdiScriptText,
	select: mdiFormatListBulleted,
	sensor: mdiEye,
	siren: mdiAlarmLight,
	switch: mdiToggleSwitch,
	update: mdiPackageUp,
	weather: mdiWeatherPartlyCloudy
};

/** Icônes de l'interface (boutons). */
export const UI = {
	close: mdiClose,
	edit: mdiPencil,
	up: mdiArrowUp,
	down: mdiArrowDown,
	remove: mdiDelete,
	add: mdiPlus,
	minus: mdiMinus,
	play: mdiPlay,
	pause: mdiPause,
	next: mdiSkipNext,
	previous: mdiSkipPrevious,
	volume: mdiVolumeHigh,
	volumeUp: mdiVolumePlus,
	volumeDown: mdiVolumeMinus
};

const BY_DEVICE_CLASS: Record<string, string> = {
	temperature: mdiThermometer,
	power: mdiFlash,
	energy: mdiFlash,
	motion: mdiMotionSensor,
	occupancy: mdiMotionSensor
};

/**
 * Chemin SVG d'une icône : nom `mdi:…` (les plus courants sont embarqués,
 * les autres chargés à la demande), sinon selon la classe d'appareil, sinon
 * selon le domaine.
 */
export function iconPath(name: unknown, entityId?: string, deviceClass?: unknown): string {
	if (typeof name === 'string' && name.startsWith('mdi:')) {
		const known = NAMED[name] ?? iconSet.get(name);
		if (known) return known;
	}
	if (typeof deviceClass === 'string' && BY_DEVICE_CLASS[deviceClass])
		return BY_DEVICE_CLASS[deviceClass];
	return (entityId ? BY_DOMAIN[domainOf(entityId)] : undefined) ?? mdiHelpCircleOutline;
}
