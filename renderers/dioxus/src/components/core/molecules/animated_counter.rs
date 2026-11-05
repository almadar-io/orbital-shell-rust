//! Dioxus renderer for the `animated-counter` pattern: the web's settled state (the value at rest, no scroll-in count-up).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, dom_passthrough, format_display_value, layout_box_spec, pattern_spec,
    DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `animated-counter` pattern.
pub struct AnimatedCounterRenderer;

impl AnimatedCounterRenderer {
    const NAME: &str = "animated-counter";
}

struct Parsed {
    num: Option<f64>,
    prefix: String,
    suffix: String,
    decimals: usize,
}

fn parse_text(text: &str) -> Parsed {
    let no_match = || Parsed {
        num: None,
        prefix: String::new(),
        suffix: text.to_string(),
        decimals: 0,
    };
    if text.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return no_match();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() && !(chars[at].is_ascii_digit() || chars[at] == '-') {
        at += 1;
    }
    let prefix: String = chars[..at].iter().collect();
    let start = at;
    if at < chars.len() && chars[at] == '-' {
        at += 1;
    }
    let digits = at;
    while at < chars.len() && chars[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return no_match();
    }
    let mut decimals = 0;
    if at + 1 < chars.len() && chars[at] == '.' && chars[at + 1].is_ascii_digit() {
        at += 1;
        let fraction = at;
        while at < chars.len() && chars[at].is_ascii_digit() {
            at += 1;
        }
        decimals = at - fraction;
    }
    let number: String = chars[start..at].iter().collect();
    let Ok(num) = number.parse::<f64>() else {
        return no_match();
    };
    Parsed {
        num: Some(num),
        prefix,
        suffix: chars[at..].iter().collect(),
        decimals,
    }
}

fn parse_value(value: Option<&Value>) -> Parsed {
    if let Some(text) = value.and_then(Value::as_str) {
        return parse_text(text);
    }
    if let Some(number) = value.and_then(Value::as_number) {
        let decimals = if number.fract() == 0.0 {
            0
        } else {
            number
                .to_string()
                .split_once('.')
                .map_or(0, |(_, fraction)| fraction.len())
        };
        return Parsed {
            num: Some(number),
            prefix: String::new(),
            suffix: String::new(),
            decimals,
        };
    }
    parse_text("")
}

fn format_number(value: f64, format: Option<&str>, decimals: usize) -> String {
    let rounded = (value + 0.5).floor();
    match format {
        Some("currency") => format_display_value(&Value::Number(value), Some("currency")),
        Some("percent") => format_display_value(&Value::Number(rounded), Some("percent")),
        Some("number") => format_display_value(&Value::Number(rounded), Some("number")),
        _ => format!("{value:.decimals$}"),
    }
}

impl DioxusPatternRenderer for AnimatedCounterRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let parsed = parse_value(props.get("value"));
        let format = get_string(props, "format");
        let text = match parsed.num {
            None => parsed.suffix,
            Some(num) => format!(
                "{}{}{}{}{}",
                get_string_or(props, "prefix", ""),
                parsed.prefix,
                format_number(num, format.as_deref(), parsed.decimals),
                parsed.suffix,
                get_string_or(props, "suffix", ""),
            ),
        };
        let mut children = vec![pattern_spec(
            "typography",
            [
                ("variant", Value::string("h2")),
                ("className", Value::string("text-primary tabular-nums")),
                ("content", Value::string(text)),
            ],
        )];
        let label = get_string_or(props, "label", "");
        if !label.is_empty() {
            children.push(pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body2")),
                    ("color", Value::string("muted")),
                    ("className", Value::string("text-center")),
                    ("content", Value::string(label)),
                ],
            ));
        }
        let class_name = get_string_or(props, "className", "");
        let class = cn(&["flex flex-col items-center gap-1", class_name.as_str()]);
        registry.render_spec(
            &layout_box_spec(&class, None, dom_passthrough(props), children),
            None,
            view,
        )
    }
}
