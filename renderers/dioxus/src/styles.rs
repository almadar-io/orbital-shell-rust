//! The native renderer uses the same compiled utility stylesheet as the web client.
//! Preset themes remain available to explicitly themed hosts.

use orbital_core::Value;

const BASE: &str = include_str!("../assets/css/almadar.css");
const WEB_CLIENT: &str = include_str!("../assets/css/web-client.css");

pub fn stylesheets() -> Vec<String> {
    vec![WEB_CLIENT.to_string(), BASE.to_string()]
}

pub fn program_stylesheets(schema: &orbital_core::OrbitalSchema) -> Vec<String> {
    let declared = schema.theme_definitions.as_deref().unwrap_or_default();
    vec![
        orbital_core::runtime::theme::program_themes_css(declared),
        WEB_CLIENT.to_string(),
        BASE.to_string(),
    ]
}

pub fn preset_stylesheets() -> Vec<String> {
    let presets = orbital_core::runtime::theme::ThemePresets::installed()
        .definitions()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    vec![
        orbital_core::runtime::theme::program_themes_css(&presets),
        WEB_CLIENT.to_string(),
        BASE.to_string(),
    ]
}

pub(crate) fn style_with_props(base: &str, props: &Value) -> String {
    let mut style = base.trim_end_matches(';').to_string();
    let Some(fields) = props
        .as_object()
        .and_then(|props| props.get("style"))
        .and_then(Value::as_object)
    else {
        return style;
    };
    for (name, value) in fields.iter() {
        let Some(value) = value.as_str() else {
            continue;
        };
        let Some(name) = css_property_name(name.as_ref()) else {
            continue;
        };
        if !style.is_empty() {
            style.push(';');
        }
        style.push_str(&name);
        style.push(':');
        style.push_str(value);
    }
    style
}

fn css_property_name(name: &str) -> Option<String> {
    if name == "cssFloat" {
        return Some("float".to_string());
    }
    if name == "--" || name == "-" {
        return None;
    }
    let is_custom = name.starts_with("--");
    let mut chars = name.chars();
    if !is_custom && !chars.next()?.is_ascii_alphabetic() && !name.starts_with('-') {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    if is_custom {
        return Some(name.to_string());
    }
    let mut property = String::with_capacity(name.len() + 4);
    if name.starts_with("Webkit")
        || name.starts_with("Moz")
        || name.starts_with("ms") && name.chars().nth(2).is_some_and(|c| c.is_ascii_uppercase())
        || name.starts_with('O') && name.chars().nth(1).is_some_and(|c| c.is_ascii_uppercase())
    {
        property.push('-');
    }
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index != 0 {
                property.push('-');
            }
            property.push(character.to_ascii_lowercase());
        } else {
            property.push(character);
        }
    }
    Some(property)
}

#[cfg(test)]
mod tests {
    use super::{preset_stylesheets, program_stylesheets, style_with_props};
    use orbital_core::{Value, ValueMap};

    fn schema_with_themes(themes: &str) -> orbital_core::OrbitalSchema {
        orbital_core::schema::parse_schema(&format!(
            r#"{{"name":"t","orbitals":[],"themeDefinitions":{themes}}}"#
        ))
        .expect("schema parses")
    }

    #[test]
    fn a_program_paints_the_themes_it_declares() {
        let preset = orbital_core::runtime::theme::ThemePresets::installed()
            .get("almadar")
            .cloned()
            .expect("almadar preset installed");
        let themes = serde_json::to_string(&vec![preset]).expect("theme serializes");
        let css = &program_stylesheets(&schema_with_themes(&themes))[0];
        assert!(css.contains(r#"[data-theme="almadar-dark"]"#));
        assert!(css.contains(r#"[data-theme="almadar-light"]"#));
        assert!(!css.contains(r#"[data-theme="minimalist-light"]"#));
    }

    #[test]
    fn preset_stylesheets_paint_every_installed_preset() {
        let css = &preset_stylesheets()[0];
        assert!(css.contains(r#"[data-theme="almadar-dark"]"#));
        assert!(css.contains(r#"[data-theme="minimalist-light"]"#));
        let declared = &program_stylesheets(&schema_with_themes("[]"))[0];
        assert!(!declared.contains(r#"[data-theme="almadar-dark"]"#));
    }

    #[test]
    fn a_program_declaring_no_theme_paints_the_default_preset() {
        let css = &program_stylesheets(&schema_with_themes("[]"))[0];
        assert!(css.contains(r#"[data-theme="minimalist-light"]"#));
        assert!(!css.contains(r#"[data-theme="almadar-dark"]"#));
    }

    #[test]
    fn projects_authored_css_properties_after_component_defaults() {
        let mut authored = ValueMap::new();
        authored.insert("minHeight".into(), Value::String("2rem".into()));
        authored.insert("--panelTone".into(), Value::String("red".into()));
        authored.insert("WebkitLineClamp".into(), Value::String("2".into()));
        let mut props = ValueMap::new();
        props.insert("style".into(), Value::Object(authored.into()));
        let projected = style_with_props("min-height:1rem", &Value::Object(props.into()));
        assert!(projected.starts_with("min-height:1rem;"));
        assert!(projected.contains("min-height:2rem"));
        assert!(projected.contains("--panelTone:red"));
        assert!(projected.contains("-webkit-line-clamp:2"));
        assert!(!projected.contains("minHeight"));
    }

    #[test]
    fn ignores_values_outside_the_declared_string_style_contract() {
        let mut authored = ValueMap::new();
        authored.insert("width".into(), Value::Int(10));
        authored.insert("color".into(), Value::String("blue".into()));
        let mut props = ValueMap::new();
        props.insert("style".into(), Value::Object(authored.into()));
        assert_eq!(
            style_with_props("", &Value::Object(props.into())),
            "color:blue"
        );
    }
}
