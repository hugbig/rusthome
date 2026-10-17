//! Prints the JSON Schema of every API type, consumed by `just gen-types`
//! to generate `frontend/src/lib/api-types.ts` (single source of truth).

use rusthome_model::{
    Area, Context, Device, EntityEntry, EntityStatus, Floor, Label, State, StateChanged,
};
use schemars::generate::SchemaSettings;

fn main() {
    // Serialize contract: describes what the API *emits* (defaulted fields are always present).
    let mut generator = SchemaSettings::draft07().for_serialize().into_generator();
    generator.subschema_for::<Context>();
    generator.subschema_for::<State>();
    generator.subschema_for::<StateChanged>();
    generator.subschema_for::<Floor>();
    generator.subschema_for::<Area>();
    generator.subschema_for::<Label>();
    generator.subschema_for::<Device>();
    generator.subschema_for::<EntityEntry>();
    generator.subschema_for::<EntityStatus>();
    let root = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "RustHomeApi",
        "definitions": generator.take_definitions(true),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&root).unwrap_or_default()
    );
}
