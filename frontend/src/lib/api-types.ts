/* Généré par 'just gen-types' depuis crates/rusthome-model : ne pas éditer. */

/**
 * A validated `domain.object_id` identifier, e.g. `light.salon`.
 *
 * Same rules as Home Assistant: lowercase ASCII letters, digits and `_`,
 * exactly one dot, no leading/trailing underscore in either part.
 *
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "EntityId".
 */
export type EntityId = string;
/**
 * Lifecycle of a registry entry (cf. inventaire/05: ~300 orphans in HA).
 *
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "EntityStatus".
 */
export type EntityStatus = 'active' | 'orphan';

export interface RustHomeApi {
	[k: string]: unknown;
}
/**
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "Area".
 */
export interface Area {
	aliases: string[];
	floor_id: string | null;
	icon: string | null;
	id: string;
	name: string;
}
/**
 * Origin of a change: used for traces, logbook and permissions.
 *
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "Context".
 */
export interface Context {
	id: string;
	parent_id?: string | null;
	user_id?: string | null;
}
/**
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "Device".
 */
export interface Device {
	area_id: string | null;
	disabled: boolean;
	id: string;
	/**
	 * Integration that owns the device (`mqtt`, `esphome`, `ha_bridge`…).
	 */
	integration: string;
	labels: string[];
	manufacturer: string | null;
	model: string | null;
	name: string;
	sw_version: string | null;
}
/**
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "EntityEntry".
 */
export interface EntityEntry {
	/**
	 * Own area; when `None` the device area applies.
	 */
	area_id: string | null;
	device_id: string | null;
	disabled: boolean;
	entity_id: EntityId;
	hidden: boolean;
	icon: string | null;
	integration: string;
	labels: string[];
	name: string | null;
	status: EntityStatus & string;
	/**
	 * Stable internal id, survives renames of `entity_id`.
	 */
	uid: string;
	/**
	 * Identifier of the entity inside its integration.
	 */
	unique_id: string | null;
}
/**
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "Floor".
 */
export interface Floor {
	icon: string | null;
	id: string;
	level: number | null;
	name: string;
}
/**
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "Label".
 */
export interface Label {
	color: string | null;
	icon: string | null;
	id: string;
	name: string;
}
/**
 * State of one entity, with Home Assistant semantics (`unavailable`,
 * `unknown` are plain string states) so imported templates keep working.
 *
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "State".
 */
export interface State {
	attributes: {
		[k: string]: unknown;
	};
	context: Context;
	entity_id: EntityId;
	last_changed: string;
	last_updated: string;
	state: string;
}
/**
 * Payload of a `state_changed` event.
 *
 * This interface was referenced by `RustHomeApi`'s JSON-Schema
 * via the `definition` "StateChanged".
 */
export interface StateChanged {
	entity_id: EntityId;
	new_state: State | null;
	old_state: State | null;
}
