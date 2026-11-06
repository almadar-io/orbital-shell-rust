use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number, get_string, get_string_or};
use orbital_core::Value;

pub struct StatBadgeRenderer;

fn size_class(size: &str) -> &'static str {
    match size {
        "xs" => "text-xs px-1.5 py-0.5",
        "sm" => "text-xs px-2 py-1",
        "lg" => "text-base px-4 py-2",
        _ => "text-sm px-3 py-1.5",
    }
}

fn variant_class(variant: &str) -> &'static str {
    match variant {
        "primary" => "bg-primary/15 border-primary/40 text-foreground",
        "success" => "bg-success/15 border-success/40 text-foreground",
        "warning" => "bg-warning/15 border-warning/40 text-foreground",
        "danger" => "bg-error/15 border-error/40 text-foreground",
        _ => "bg-card/80 border-border text-foreground",
    }
}

fn js_parse_int(text: &str) -> f64 {
    let trimmed = text.trim_start();
    let mut end = 0;
    for (index, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() || (index == 0 && (ch == '-' || ch == '+')) {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    trimmed[..end].parse::<f64>().unwrap_or(0.0)
}

impl DioxusPatternRenderer for StatBadgeRenderer {
    fn pattern_name(&self) -> &str {
        "stat-badge"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let label = get_string_or(props, "label", "");
        let size = get_string_or(props, "size", "md");
        let variant = get_string_or(props, "variant", "default");
        let format = get_string_or(props, "format", "number");
        let extra = get_string_or(props, "className", "");
        let class = cn(&[
            BOX_ATOM_BASE,
            &cn(&[
                "inline-flex items-center gap-2 rounded-container border backdrop-blur-sm",
                size_class(&size),
                variant_class(&variant),
                extra.trim(),
            ]),
        ]);
        let fields = props.as_object();
        let value = fields
            .and_then(|fields| fields.get("value"))
            .filter(|value| !value.is_null());
        let number = match value {
            Some(Value::Int(number)) => *number as f64,
            Some(Value::Number(number)) => *number,
            Some(other) => js_parse_int(&other.to_string_value()),
            None => 0.0,
        };
        let max = get_number(props, "max").filter(|max| *max != 0.0);
        let asset = fields
            .and_then(|fields| fields.get("iconUrl").or_else(|| fields.get("assetUrl")))
            .filter(|asset| !asset.is_null());
        let bar_size = if size == "lg" { "md" } else { "sm" };

        let lead = if let Some(asset) = asset {
            let spec = pattern_spec(
                "game-icon",
                [
                    ("assetUrl", asset.clone()),
                    ("icon", Value::string("image")),
                    ("size", Value::Number(16.0)),
                    ("className", Value::string("flex-shrink-0")),
                ],
            );
            registry.render_spec(&spec, entity, view)
        } else if let Some(name) = get_string(props, "icon").filter(|name| !name.is_empty()) {
            let spec = pattern_spec(
                "icon",
                [
                    ("name", Value::string(name.as_str())),
                    ("className", Value::string("w-4 h-4")),
                ],
            );
            let icon = registry.render_spec(&spec, entity, view);
            rsx! { span { class: "flex-shrink-0 text-lg", {icon} } }
        } else {
            rsx! {}
        };

        let label_spec = pattern_spec(
            "typography",
            [
                ("as", Value::string("span")),
                (
                    "className",
                    Value::string("text-muted-foreground font-medium text-xs"),
                ),
                ("content", Value::string(label.as_str())),
            ],
        );
        let label_node = registry.render_spec(&label_spec, entity, view);

        let display = match value {
            None => rsx! {},
            Some(value) => match (format.as_str(), max) {
                ("hearts" | "bar", Some(max)) => {
                    let spec = pattern_spec(
                        "health-bar",
                        [
                            ("current", Value::Number(number)),
                            ("max", Value::Number(max)),
                            ("format", Value::string(format.as_str())),
                            ("size", Value::string(bar_size)),
                        ],
                    );
                    registry.render_spec(&spec, entity, view)
                }
                ("number", _) => {
                    let mut score = vec![
                        ("value", Value::Number(number)),
                        ("size", Value::string(bar_size)),
                        ("className", Value::string("font-display")),
                    ];
                    if let Some(text) = get_string(props, "displayValue") {
                        score.push(("displayValue", Value::string(text.as_str())));
                    }
                    let spec = pattern_spec("score-display", score);
                    registry.render_spec(&spec, entity, view)
                }
                ("text", _) => {
                    let spec = pattern_spec(
                        "typography",
                        [
                            ("as", Value::string("span")),
                            ("className", Value::string("font-bold text-foreground")),
                            ("content", Value::string(value.to_string_value().as_str())),
                        ],
                    );
                    registry.render_spec(&spec, entity, view)
                }
                _ => rsx! {},
            },
        };

        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                {lead}
                {label_node}
                {display}
            }
        }
    }
}
