use orbital_core::client::helpers::to_json;
use orbital_core::schema::OrbitalSchema;
use orbital_core::Value;

pub(crate) fn schema_from_props(props: &Value) -> Option<OrbitalSchema> {
    let schema = props.as_object()?.get("schema")?;
    match schema {
        Value::String(text) => serde_json::from_str(text.as_str()).ok(),
        other => serde_json::from_value(to_json(other)).ok(),
    }
}
