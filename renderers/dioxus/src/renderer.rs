//! Dioxus pattern-renderer contract.
//!
//! Mirrors `orbital-client/src/rendering/desktop.rs:42-54` (`PatternRenderer`)
//! but returns a Dioxus [`Element`] (RSX, retained VDOM) instead of painting
//! into `&mut egui::Ui` (immediate-mode).
//!
//! This contract is intentionally *separate* from the shared [`RenderContext`]
//! (which is `&mut`-heavy and egui-shaped). Dioxus renderers are pure functions
//! `props → Element`: they read entity/trait data from a read-only [`DioxusView`],
//! emit user events through an [`EventSink`] (a `Callback` the root `App` wires
//! into `ClientCore::send_event`), and recurse into children through the
//! [`DioxusPatternRegistry`] (no `ui_ptr`, no `&mut`). Nothing here touches the
//! egui substrate — the shared `RenderContext` is left byte-identical.

use crate::registry::DioxusPatternRegistry;
use dioxus::prelude::*;
use orbital_core::client::recurse::trait_ref_name;
use orbital_core::Value;
use std::collections::HashMap;

/// Event sink a Dioxus control calls to dispatch a user event back into the
/// Orbital circuit. `(event_name, payload)` — `Callback` is `Copy`, so controls
/// capture it inside `onclick: move |_| …` without cloning.
pub type EventSink = Callback<(String, Value)>;
pub type SourceSinkFactory = Callback<Option<String>, EventSink>;

/// Dispatch `payload` under the declared event `event`, when the pattern names one.
pub(crate) fn emit_event(sink: Option<EventSink>, event: &Option<String>, payload: Value) {
    if let (Some(sink), Some(event)) = (sink, event) {
        sink.call((event.clone(), payload));
    }
}

/// Where a mouse event fell on its target element, and that element's laid-out size: a drag reads its position as a
/// share of a childless overlay it lays over its container. Only the Blitz shell reports the size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TargetPoint {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub(crate) fn target_point(event: &MouseEvent) -> Option<TargetPoint> {
    #[cfg(feature = "native")]
    {
        let data = event.data();
        let native = data.downcast::<dioxus_native::NativeClickData>()?;
        let (width, height) = native.target_size();
        let at = data.element_coordinates();
        Some(TargetPoint { x: at.x, y: at.y, width, height })
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = event;
        None
    }
}

pub(crate) fn emit_empty_event(sink: Option<EventSink>, name: Option<&str>) {
    if let (Some(sink), Some(name)) = (sink, name.filter(|name| !name.is_empty())) {
        sink.call((name.to_string(), Value::Null));
    }
}

/// A row action's route; a route that fails to evaluate is logged and the action draws no link.
pub(crate) fn logged_row_route(action: &Value, row: &Value) -> Option<String> {
    orbital_core::client::helpers::row_action_route(action, row, None).unwrap_or_else(|error| {
        tracing::error!(%error, "row action route");
        None
    })
}

/// A row action's payload; a payload that fails to evaluate is logged and the action is not drawn or dispatched.
pub(crate) fn logged_row_payload(action: &Value, row: &Value) -> Option<Value> {
    orbital_core::client::helpers::row_action_payload(action, row, None)
        .map_err(|error| tracing::error!(%error, "row action payload"))
        .ok()
}

/// A sub-pattern spec `{ type: name, ..fields }` a body hands to `registry.render_spec`, so it composes the
/// pattern's own renderer instead of re-drawing it.
pub(crate) fn pattern_spec<K: AsRef<str>>(name: &str, fields: impl IntoIterator<Item = (K, Value)>) -> Value {
    let mut spec = orbital_core::ValueMap::new();
    spec.insert("type".into(), Value::string(name));
    for (key, value) in fields {
        spec.insert(key.as_ref().into(), value);
    }
    Value::Object(spec.into())
}

/// An object value from `(key, value)` pairs (an event payload, a nested prop).
pub(crate) fn object_value<K: AsRef<str>>(fields: impl IntoIterator<Item = (K, Value)>) -> Value {
    let mut map = orbital_core::ValueMap::new();
    for (key, value) in fields {
        map.insert(key.as_ref().into(), value);
    }
    Value::Object(map.into())
}

/// An array value (a `children` list) from specs.
pub(crate) fn array_value(items: Vec<Value>) -> Value {
    Value::Array(items.into())
}

/// The web's `domPassthrough`: the `aria-*`, `data-*`, `role`, `id`, `lang`, `dir` and `tabIndex` props a layout primitive forwards to its element.
pub(crate) fn dom_passthrough(props: &Value) -> Vec<(String, Value)> {
    let Some(fields) = props.as_object() else {
        return Vec::new();
    };
    fields
        .iter()
        .filter(|(key, value)| {
            let key: &str = key.as_ref();
            (key.starts_with("aria-")
                || key.starts_with("data-")
                || matches!(key, "role" | "id" | "lang" | "dir" | "tabIndex"))
                && matches!(value, Value::String(_) | Value::Int(_) | Value::Bool(_))
        })
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

/// A navigation landmark's forwarded attributes: `role="navigation"` and the default `aria-label`, with authored `aria-*`/`role` values taking over, as the web's `role=... aria-label=... {...domPassthrough(rest)}`.
pub(crate) fn nav_landmark(props: &Value, default_label: &str) -> Vec<(String, Value)> {
    let mut fields = vec![
        ("role".to_string(), Value::string("navigation")),
        ("aria-label".to_string(), Value::string(default_label)),
    ];
    for (key, value) in dom_passthrough(props) {
        fields.retain(|(existing, _)| *existing != key);
        fields.push((key, value));
    }
    fields
}

/// Doc navigation row states shared by the doc-sidebar, doc-toc and doc-pagination bodies (web `lib/doc-nav-classes.ts`).
pub(crate) const DOC_NAV_ROW_HOVER: &str = "hover:bg-muted hover:text-foreground hover:border-transparent";
pub(crate) const DOC_NAV_ROW_ACTIVE: &str = "bg-accent/10 text-accent font-semibold hover:bg-accent/15 hover:text-accent hover:border-transparent";
pub(crate) const DOC_NAV_ROW_IDLE: &str = "text-muted-foreground";
pub(crate) const DOC_NAV_RULE_HOVER: &str = "hover:bg-transparent hover:text-foreground";
pub(crate) const DOC_NAV_CARD_HOVER: &str = "hover:bg-card hover:text-foreground hover:border-accent";

/// A `typography` spec for the text a body writes where the web writes `<Typography>`.
pub(crate) fn typography_spec(variant: &str, color: Option<&str>, class: &str, content: &str) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("content", Value::string(content)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if !class.is_empty() {
        fields.push(("className", Value::string(class)));
    }
    pattern_spec("typography", fields)
}

/// Joins the non-empty class fragments, as the web's `cn(...)`.
/// A body's base classes merged with its `className` prop the way the web's `cn` does (tailwind-merge: a later
/// conflicting utility replaces the earlier one).
pub fn merge_class_name(base: &str, props: &Value) -> String {
    let extra = orbital_core::client::helpers::get_string_or(props, "className", "");
    cn(&[base, extra.trim()])
}

pub(crate) fn cn(parts: &[&str]) -> String {
    let joined = parts.iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ");
    crate::tw_merge::tw_merge(&joined)
}

/// A `box` spec for the web's `<Box className=...>`: `extra` carries forwarded DOM attributes, `style` an optional style object.
pub(crate) fn layout_box_spec(
    class: &str,
    style: Option<Value>,
    extra: Vec<(String, Value)>,
    children: Vec<Value>,
) -> Value {
    let mut fields = extra;
    if !class.is_empty() {
        fields.push(("className".to_string(), Value::string(class)));
    }
    if let Some(style) = style {
        fields.push(("style".to_string(), style));
    }
    fields.push(("children".to_string(), array_value(children)));
    pattern_spec("box", fields)
}

/// A row field read by dot path (`a.b.c`), as the web's `getNestedValue` does.
pub(crate) fn nested_value(row: &Value, path: &str) -> Option<Value> {
    let mut current = row.clone();
    for part in path.split('.') {
        let next = current.as_object()?.get(part)?.clone();
        current = next;
    }
    Some(current)
}

fn group_thousands(digits: &str) -> String {
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `Intl.NumberFormat('en')` with at least `min_fraction` and at most `max_fraction` fraction digits.
pub(crate) fn format_grouped_number(value: f64, min_fraction: usize, max_fraction: usize) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let rounded = format!("{:.*}", max_fraction, value.abs());
    let (whole, fraction) = rounded.split_once('.').unwrap_or((rounded.as_str(), ""));
    let mut fraction = fraction.to_string();
    while fraction.len() > min_fraction && fraction.ends_with('0') {
        fraction.pop();
    }
    let negative = value < 0.0 && rounded.chars().any(|ch| ch != '0' && ch != '.');
    let sign = if negative { "-" } else { "" };
    if fraction.is_empty() {
        format!("{sign}{}", group_thousands(whole))
    } else {
        format!("{sign}{}.{fraction}", group_thousands(whole))
    }
}

fn date_parts(text: &str) -> Option<(i32, u32, u32, Option<(u32, u32)>)> {
    let (date, time) = match text.split_once(['T', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let clock = time.and_then(|time| {
        let mut parts = time.split([':', '.', 'Z', '+']);
        let hour: u32 = parts.next()?.parse().ok()?;
        let minute: u32 = parts.next()?.parse().ok()?;
        (hour < 24 && minute < 60).then_some((hour, minute))
    });
    Some((year, month, day, clock))
}

fn month_name(month: u32) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month as usize - 1) % 12]
}

fn clock_text(hour: u32, minute: u32) -> String {
    let suffix = if hour < 12 { "AM" } else { "PM" };
    let hour12 = match hour % 12 {
        0 => 12,
        other => other,
    };
    format!("{hour12}:{minute:02} {suffix}")
}

/// The stored text of a value, as JavaScript's `String(value)` reads it.
pub(crate) fn plain_text(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                other => plain_text(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
        Value::String(text) => text.to_string(),
        Value::Int(whole) => whole.to_string(),
        Value::Number(float) => float.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

/// The web's shared `formatValue(value, format)`: a declared format shapes the value; booleans always read Yes/No; no format is the stored text.
pub(crate) fn format_display_value(value: &Value, format: Option<&str>) -> String {
    let number = match value {
        Value::Int(whole) => Some(*whole as f64),
        Value::Number(float) => Some(*float),
        _ => None,
    };
    match value {
        Value::Null => return String::new(),
        Value::Bool(flag) => {
            return if *flag {
                crate::i18n::t("common.yes")
            } else {
                crate::i18n::t("common.no")
            }
        }
        _ => {}
    }
    let text = plain_text(value);
    match format {
        Some(kind @ ("date" | "time" | "datetime")) => {
            if text.is_empty() {
                return text;
            }
            let Some((year, month, day, clock)) = date_parts(&text) else {
                return text;
            };
            let date = format!("{} {day}, {year}", month_name(month));
            let time = clock.map(|(hour, minute)| clock_text(hour, minute));
            match (kind, time) {
                ("date", _) => date,
                ("time", Some(time)) => time,
                ("datetime", Some(time)) => format!("{date} {time}"),
                ("datetime", None) => format!("{date} 12:00 AM"),
                _ => text,
            }
        }
        Some("currency") => match number {
            Some(amount) => {
                let body = format_grouped_number(amount.abs(), 2, 2);
                if amount < 0.0 && body != "0.00" {
                    format!("-${body}")
                } else {
                    format!("${body}")
                }
            }
            None => text,
        },
        Some("number") => match number {
            Some(amount) => format_grouped_number(amount, 0, 3),
            None => text,
        },
        Some("percent") => match number {
            Some(amount) => format!("{}%", format_grouped_number(amount, 0, 0)),
            None => text,
        },
        Some("boolean") => {
            let falsy = text == "false" || text == "0";
            if falsy {
                crate::i18n::t("common.no")
            } else {
                crate::i18n::t("common.yes")
            }
        }
        _ => text,
    }
}

fn relation_option_label(options: Option<&[Value]>, key: &str) -> Option<String> {
    options?
        .iter()
        .find(|option| option.get("value").map(plain_text).as_deref() == Some(key))
        .and_then(|option| option.get("label").map(plain_text))
}

fn relation_labels(value: &Value, options: Option<&[Value]>, out: &mut Vec<String>) {
    match value {
        Value::Null => {}
        Value::String(text) if text.is_empty() => {}
        Value::Array(items) => items.iter().for_each(|item| relation_labels(item, options, out)),
        Value::Object(_) => {
            if let Some(id) = value.get("id").filter(|id| !id.is_null()) {
                let key = plain_text(&id);
                out.push(relation_option_label(options, &key).unwrap_or(key));
            }
        }
        scalar => {
            let raw = plain_text(scalar);
            out.push(relation_option_label(options, &raw).unwrap_or(raw));
        }
    }
}

/// The web's `resolveRelationCellDisplay`: a hydrated relation row, or a bare foreign id with declared `relationsData` options, reads by label.
pub(crate) fn relation_cell_display(value: &Value, options: Option<&[Value]>) -> Option<String> {
    let object_shaped = match value {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().any(|item| matches!(item, Value::Object(_))),
        _ => false,
    };
    if matches!(value, Value::Null) || matches!(value, Value::String(text) if text.is_empty()) {
        return None;
    }
    if !object_shaped && options.is_none() {
        return None;
    }
    let mut labels = Vec::new();
    relation_labels(value, options, &mut labels);
    (!labels.is_empty()).then(|| labels.join(", "))
}

/// A field's display text: relation label first, then the declared format.
pub(crate) fn format_cell_text(value: &Value, format: Option<&str>, options: Option<&[Value]>) -> String {
    relation_cell_display(value, options).unwrap_or_else(|| format_display_value(value, format))
}

/// The web `String(value)` of a scalar field value.
pub(crate) fn js_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Int(number) => number.to_string(),
        Value::Number(number) => {
            if number.fract() == 0.0 && number.abs() < 1e21 {
                format!("{}", *number as i64)
            } else {
                number.to_string()
            }
        }
        Value::String(text) => text.to_string(),
        other => orbital_core::client::helpers::format_value(other),
    }
}

fn grouped_number(number: f64, max_fraction: usize, min_fraction: usize) -> String {
    let negative = number < 0.0;
    let fixed = format!("{:.*}", max_fraction, number.abs());
    let (int_part, frac_part) = match fixed.split_once('.') {
        Some((int_part, frac)) => (int_part.to_string(), frac.to_string()),
        None => (fixed.clone(), String::new()),
    };
    let mut frac = frac_part;
    while frac.len() > min_fraction && frac.ends_with('0') {
        frac.pop();
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&group_thousands(&int_part));
    if !frac.is_empty() {
        out.push('.');
        out.push_str(&frac);
    }
    out
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn iso_parts(text: &str) -> Option<(i64, usize, i64, Option<(i64, i64)>)> {
    let (date, time) = match text.split_once(['T', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<usize>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&month) || parts.next().is_some() {
        return None;
    }
    let clock = time.and_then(|time| {
        let mut parts = time.split([':', '.', 'Z', '+']);
        let hour = parts.next()?.parse::<i64>().ok()?;
        let minute = parts.next()?.parse::<i64>().ok()?;
        Some((hour, minute))
    });
    Some((year, month, day, clock))
}

fn clock_label(hour: i64, minute: i64) -> String {
    let suffix = if hour >= 12 { "PM" } else { "AM" };
    let hour12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{hour12}:{minute:02} {suffix}")
}

/// The web `formatValue(value, format)` in the en-US, USD default context.
pub(crate) fn format_field_value(value: &Value, format: Option<&str>) -> String {
    match value {
        Value::Null => return String::new(),
        Value::Bool(flag) => {
            return if *flag {
                crate::i18n::t("common.yes")
            } else {
                crate::i18n::t("common.no")
            }
        }
        _ => {}
    }
    let raw = js_string(value);
    let numeric = value.as_number().filter(|_| value.is_numeric());
    match format {
        Some("date") | Some("datetime") | Some("time") => {
            if raw.is_empty() {
                return String::new();
            }
            let Some((year, month, day, clock)) = iso_parts(&raw) else {
                return raw;
            };
            let date = format!("{} {day}, {year}", MONTHS[month - 1]);
            match (format, clock) {
                (Some("date"), _) => date,
                (Some("time"), Some((hour, minute))) => clock_label(hour, minute),
                (Some("datetime"), Some((hour, minute))) => format!("{date} {}", clock_label(hour, minute)),
                (Some("datetime"), None) => format!("{date} 12:00 AM"),
                _ => raw,
            }
        }
        Some("currency") => match numeric {
            Some(number) => {
                let body = grouped_number(number.abs(), 2, 2);
                if number < 0.0 {
                    format!("-${body}")
                } else {
                    format!("${body}")
                }
            }
            None => raw,
        },
        Some("number") => match numeric {
            Some(number) => grouped_number(number, 3, 0),
            None => raw,
        },
        Some("percent") => match numeric {
            Some(number) => format!("{}%", grouped_number(number, 0, 0)),
            None => raw,
        },
        _ => raw,
    }
}

/// The label of a hydrated relation row: its injected option label for its id, else its id.
pub(crate) fn relation_label(value: &Value, options: Option<&Value>) -> Option<String> {
    let Value::Object(_) = value else { return None };
    let id = value.get("id").filter(|id| !id.is_null())?;
    let key = js_string(id);
    let label = options
        .and_then(Value::as_array)
        .and_then(|options| {
            options
                .iter()
                .find(|option| option.get("value").map(js_string).as_deref() == Some(key.as_str()))
        })
        .and_then(|option| option.get("label"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(label.unwrap_or(key))
}

/// Every display label a relation-shaped value resolves to.
pub(crate) fn relation_display_labels(value: &Value, options: Option<&Value>) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::String(text) if text.is_empty() => Vec::new(),
        Value::Array(items) => items
            .iter()
            .flat_map(|item| relation_display_labels(item, options))
            .collect(),
        other => {
            if let Some(label) = relation_label(other, options) {
                return vec![label];
            }
            let raw = js_string(other);
            let matched = options.and_then(Value::as_array).and_then(|options| {
                options
                    .iter()
                    .find(|option| option.get("value").map(js_string).as_deref() == Some(raw.as_str()))
                    .and_then(|option| option.get("label"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            vec![matched.unwrap_or(raw)]
        }
    }
}

pub type NavigationSink = Callback<String>;

#[derive(Clone, Copy, PartialEq)]
pub enum DockCommand {
    ShowPanel,
    CloseMenu,
}

#[derive(Clone, PartialEq)]
pub struct DockContext {
    pub compact: bool,
    pub command: Callback<DockCommand>,
}

/// Read-only view handed to a Dioxus renderer. Carries the immutable subset of
/// the shared `RenderContext` renderers actually need (entity cache, `@trait`
/// content, current state) plus the [`EventSink`] for dispatch. `source_trait`
/// mirrors `RenderContext::source_trait` so a button nested under `@trait.X`
/// emits with source `X`.
pub struct DioxusView<'a> {
    /// Name of the currently active trait.
    pub trait_name: &'a str,
    /// Name of the currently active orbital.
    pub orbital_name: &'a str,
    /// Name of the current state machine state.
    pub current_state: &'a str,
    /// Cached entity data returned by the server.
    pub entities: &'a HashMap<String, Vec<Value>>,
    /// Per-trait current render output, keyed by trait name (for `@trait.X`).
    pub trait_content: &'a orbital_core::ValueMap,
    /// The trait that authored the content currently being rendered — the source
    /// of any event a control here dispatches.
    pub source_trait: Option<String>,
    /// Event sink wired by the root `App` into `ClientCore::send_event`.
    pub sink: Option<EventSink>,
    pub source_sink: SourceSinkFactory,
    pub navigate: Option<NavigationSink>,
    pub dock: Option<DockContext>,
    pub viewport_width: Option<f64>,
    /// The kernel's `canvasTheme`, handed to every Rust-drawn scene (`None` without a kernel).
    pub canvas_theme: Option<serde_json::Value>,
    pub theme_mode: Option<orbital_core::ThemeMode>,
    pub toggle_theme: Option<Callback<()>>,
}

/// A named UI pattern rendered to a Dioxus subtree.
impl<'a> DioxusView<'a> {
    /// This view for content `source` rendered, dispatching through `sink` (bound to it).
    pub fn for_source(&self, source: Option<String>, sink: EventSink) -> DioxusView<'a> {
        DioxusView {
            trait_name: self.trait_name,
            orbital_name: self.orbital_name,
            current_state: self.current_state,
            entities: self.entities,
            trait_content: self.trait_content,
            source_trait: source,
            sink: Some(sink),
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
        }
    }

    pub fn for_trait_source(&self, source: String) -> DioxusView<'a> {
        let sink = self.source_sink.call(Some(source.clone()));
        self.for_source(Some(source), sink)
    }

    pub fn render_child(
        &self,
        registry: &DioxusPatternRegistry,
        child: &Value,
        source: Option<&str>,
    ) -> Element {
        if orbital_core::client::helpers::is_empty_layout_node(child) {
            return rsx! {};
        }
        match source {
            Some(source) => {
                registry.render_spec(child, None, &self.for_trait_source(source.to_string()))
            }
            None => registry.render_spec(child, None, self),
        }
    }

    pub fn render_node(&self, registry: &DioxusPatternRegistry, node: &Value) -> Element {
        match node {
            Value::Array(items) => rsx! {
                for item in items.iter() {
                    {self.render_node(registry, item)}
                }
            },
            Value::String(text) => {
                if let Some(source) = trait_ref_name(text) {
                    if let Some(content) = self.trait_content.get(&source) {
                        registry.render_spec(content, None, &self.for_trait_source(source))
                    } else {
                        rsx! {}
                    }
                } else {
                    rsx! { "{text}" }
                }
            }
            Value::Object(_) if orbital_core::client::helpers::is_empty_layout_node(node) => rsx! {},
            Value::Object(_) => registry.render_spec(node, None, self),
            _ => rsx! {},
        }
    }

    pub fn to_owned(&self) -> OwnedDioxusView {
        OwnedDioxusView {
            trait_name: self.trait_name.to_string(),
            orbital_name: self.orbital_name.to_string(),
            current_state: self.current_state.to_string(),
            entities: self.entities.clone(),
            trait_content: self.trait_content.clone(),
            source_trait: self.source_trait.clone(),
            sink: self.sink,
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct OwnedDioxusView {
    pub trait_name: String,
    pub orbital_name: String,
    pub current_state: String,
    pub entities: HashMap<String, Vec<Value>>,
    pub trait_content: orbital_core::ValueMap,
    pub source_trait: Option<String>,
    pub sink: Option<EventSink>,
    pub source_sink: SourceSinkFactory,
    pub navigate: Option<NavigationSink>,
    pub dock: Option<DockContext>,
    pub viewport_width: Option<f64>,
    pub canvas_theme: Option<serde_json::Value>,
    pub theme_mode: Option<orbital_core::ThemeMode>,
    pub toggle_theme: Option<Callback<()>>,
}

impl OwnedDioxusView {
    pub fn as_view(&self) -> DioxusView<'_> {
        DioxusView {
            trait_name: &self.trait_name,
            orbital_name: &self.orbital_name,
            current_state: &self.current_state,
            entities: &self.entities,
            trait_content: &self.trait_content,
            source_trait: self.source_trait.clone(),
            sink: self.sink,
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
        }
    }
}

pub trait DioxusPatternRenderer: Send + Sync {
    /// The pattern this renderer handles (e.g. "button").
    fn pattern_name(&self) -> &str;

    /// Render the pattern to a Dioxus subtree. `registry` is passed so container
    /// renderers (`stack`, grids) can recurse into their `children` by calling
    /// [`DioxusPatternRegistry::render_spec`]; leaf renderers ignore it.
    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element;
}

/// A sink for a body's own controls: `trigger` sends each of `events` (no payload) to `sink`, every other event passes through. Make it once per component (inside `use_hook`).
pub(crate) fn fanout_sink(sink: Option<EventSink>, trigger: &'static str, events: &'static [&'static str]) -> EventSink {
    Callback::new(move |(event, payload): (String, Value)| {
        let Some(sink) = sink else {
            return;
        };
        if event == trigger {
            for name in events {
                sink.call(((*name).to_string(), object_value::<&str>([])));
            }
        } else {
            sink.call((event, payload));
        }
    })
}

/// The classes the web `Box` applies for its default `bg`, `rounded` and `shadow` props.
pub(crate) const BOX_ATOM_BASE: &str = "bg-transparent rounded-none shadow-none";

/// A JS-number payload field (the web emits `number`, not an integer variant).
pub(crate) fn number_value(number: f64) -> Value {
    Value::Number(number)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LineDiffKind {
    Added,
    Removed,
    Unchanged,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct LineDiffLine {
    pub kind: LineDiffKind,
    pub before_line_number: Option<usize>,
    pub after_line_number: Option<usize>,
    pub content: String,
}

fn diff_lines_of(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    }
}

/// The web's `computeLineDiff`: Myers' O(ND) line diff with the linear-space middle snake.
pub(crate) fn compute_line_diff(before: &str, after: &str) -> Vec<LineDiffLine> {
    let a = diff_lines_of(before);
    let b = diff_lines_of(after);
    let mut ops: Vec<(LineDiffKind, usize)> = Vec::new();
    diff_range(&a, 0, a.len(), &b, 0, b.len(), &mut ops);
    let mut out = Vec::with_capacity(ops.len());
    let (mut bn, mut an) = (1usize, 1usize);
    for (kind, index) in ops {
        match kind {
            LineDiffKind::Unchanged => {
                out.push(LineDiffLine { kind, before_line_number: Some(bn), after_line_number: Some(an), content: a[index].to_string() });
                bn += 1;
                an += 1;
            }
            LineDiffKind::Removed => {
                out.push(LineDiffLine { kind, before_line_number: Some(bn), after_line_number: None, content: a[index].to_string() });
                bn += 1;
            }
            LineDiffKind::Added => {
                out.push(LineDiffLine { kind, before_line_number: None, after_line_number: Some(an), content: b[index].to_string() });
                an += 1;
            }
        }
    }
    out
}

fn diff_range(a: &[&str], mut a_lo: usize, mut a_hi: usize, b: &[&str], mut b_lo: usize, mut b_hi: usize, ops: &mut Vec<(LineDiffKind, usize)>) {
    let mut prefix = 0;
    while a_lo + prefix < a_hi && b_lo + prefix < b_hi && a[a_lo + prefix] == b[b_lo + prefix] {
        prefix += 1;
    }
    for i in 0..prefix {
        ops.push((LineDiffKind::Unchanged, a_lo + i));
    }
    a_lo += prefix;
    b_lo += prefix;
    let mut suffix = 0;
    while a_hi - suffix > a_lo && b_hi - suffix > b_lo && a[a_hi - suffix - 1] == b[b_hi - suffix - 1] {
        suffix += 1;
    }
    let tail = a_hi - suffix;
    a_hi -= suffix;
    b_hi -= suffix;
    if a_lo == a_hi {
        for j in b_lo..b_hi {
            ops.push((LineDiffKind::Added, j));
        }
    } else if b_lo == b_hi {
        for i in a_lo..a_hi {
            ops.push((LineDiffKind::Removed, i));
        }
    } else {
        let (x, y, u, v) = diff_middle_snake(a, a_lo, a_hi, b, b_lo, b_hi);
        diff_range(a, a_lo, x, b, b_lo, y, ops);
        for i in x..u {
            ops.push((LineDiffKind::Unchanged, i));
        }
        diff_range(a, u, a_hi, b, v, b_hi, ops);
    }
    for i in 0..suffix {
        ops.push((LineDiffKind::Unchanged, tail + i));
    }
}

fn diff_middle_snake(a: &[&str], a_lo: usize, a_hi: usize, b: &[&str], b_lo: usize, b_hi: usize) -> (usize, usize, usize, usize) {
    let n = (a_hi - a_lo) as i64;
    let m = (b_hi - b_lo) as i64;
    let delta = n - m;
    let odd = delta & 1 != 0;
    let max = (n + m + 1) / 2;
    let offset = max + 1;
    let mut forward = vec![0i64; (2 * max + 3) as usize];
    let mut backward = vec![0i64; (2 * max + 3) as usize];
    let at = |k: i64| (offset + k) as usize;
    for d in 0..=max {
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && forward[at(k - 1)] < forward[at(k + 1)]) { forward[at(k + 1)] } else { forward[at(k - 1)] + 1 };
            let mut y = x - k;
            let (x0, y0) = (x, y);
            while x < n && y < m && a[a_lo + x as usize] == b[b_lo + y as usize] {
                x += 1;
                y += 1;
            }
            forward[at(k)] = x;
            let kb = delta - k;
            if odd && kb >= -(d - 1) && kb <= d - 1 && x + backward[at(kb)] >= n {
                return (a_lo + x0 as usize, b_lo + y0 as usize, a_lo + x as usize, b_lo + y as usize);
            }
            k += 2;
        }
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && backward[at(k - 1)] < backward[at(k + 1)]) { backward[at(k + 1)] } else { backward[at(k - 1)] + 1 };
            let mut y = x - k;
            let (x0, y0) = (x, y);
            while x < n && y < m && a[a_hi - 1 - x as usize] == b[b_hi - 1 - y as usize] {
                x += 1;
                y += 1;
            }
            backward[at(k)] = x;
            let kf = delta - k;
            if !odd && kf >= -d && kf <= d && x + forward[at(kf)] >= n {
                return (a_hi - x as usize, b_hi - y as usize, a_hi - x0 as usize, b_hi - y0 as usize);
            }
            k += 2;
        }
    }
    unreachable!("compute_line_diff: no middle snake for differing texts")
}

/// The DOM attributes an atom forwards from its props like the web's `domPassthrough`: `shared_root_attrs` plus `title`, `data-testid` and the remaining `aria-*` values.
pub(crate) fn passthrough_attrs(props: &Value, defaults: &[(&'static str, Value)]) -> Vec<Attribute> {
    let mut attrs = crate::registry::shared_root_attrs_with_defaults(props, defaults);
    let Some(fields) = props.as_object() else {
        return attrs;
    };
    for name in ["title"] {
        if let Some(Value::String(text)) = fields.get(name) {
            attrs.push(Attribute::new(name, text.to_string(), None, false));
        }
    }
    for name in [
        "aria-checked",
        "aria-controls",
        "aria-haspopup",
        "aria-invalid",
        "aria-required",
        "aria-readonly",
        "aria-autocomplete",
        "aria-modal",
        "aria-multiline",
        "aria-multiselectable",
        "aria-orientation",
        "aria-owns",
        "aria-activedescendant",
        "aria-errormessage",
        "aria-details",
        "aria-roledescription",
        "aria-keyshortcuts",
        "aria-placeholder",
        "aria-sort",
        "aria-atomic",
        "aria-relevant",
        "aria-level",
        "aria-valuenow",
        "aria-valuemin",
        "aria-valuemax",
        "aria-valuetext",
    ] {
        let text = match fields.get(name) {
            Some(Value::String(text)) => text.to_string(),
            Some(Value::Bool(flag)) => flag.to_string(),
            Some(Value::Int(number)) => number.to_string(),
            _ => continue,
        };
        attrs.push(Attribute::new(name, text, None, false));
    }
    attrs
}

/// Renders `spec` with a local sink: events for which `handle(event, payload)` returns true stay with the caller, every other event goes to the view's sink with the view's source.
pub(crate) fn render_with_local_sink(
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
    spec: &Value,
    handle: impl Fn(&str, &Value) -> bool + 'static,
) -> Element {
    let parent = view.sink;
    let relay: EventSink = Callback::new(move |(event, payload): (String, Value)| {
        if handle(&event, &payload) {
            return;
        }
        if let Some(parent) = parent {
            parent.call((event, payload));
        }
    });
    registry.render_spec(spec, None, &view.for_source(view.source_trait.clone(), relay))
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum PatternTileElement {
    Path {
        d: String,
        #[serde(default)]
        opacity: Option<f64>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        #[serde(default)]
        filled: bool,
        #[serde(default)]
        opacity: Option<f64>,
    },
}

#[derive(serde::Deserialize)]
pub(crate) struct PatternTileVariant {
    pub height: f64,
    pub elements: Vec<PatternTileElement>,
}

#[derive(serde::Deserialize)]
pub(crate) struct PatternTileGeometry {
    pub unit: f64,
    pub variants: HashMap<String, PatternTileVariant>,
}

/// The `pattern-tile` geometry the installed pattern contract declares (the web's `pattern-tile-geometry.json`).
pub(crate) fn pattern_tile_geometry() -> Result<&'static PatternTileGeometry, String> {
    static GEOMETRY: std::sync::OnceLock<PatternTileGeometry> = std::sync::OnceLock::new();
    if let Some(geometry) = GEOMETRY.get() {
        return Ok(geometry);
    }
    let installed = orbital_core::language_data::language_data()
        .map_err(|error| format!("pattern-tile geometry: {error}"))?;
    let registry: serde_json::Value = serde_json::from_str(&installed.patterns)
        .map_err(|error| format!("pattern registry: {error}"))?;
    let source = registry
        .pointer("/patterns/pattern-tile/geometry")
        .cloned()
        .ok_or_else(|| "pattern-tile geometry missing from installed contract".to_string())?;
    let parsed: PatternTileGeometry = serde_json::from_value(source)
        .map_err(|error| format!("pattern-tile geometry invalid: {error}"))?;
    Ok(GEOMETRY.get_or_init(|| parsed))
}

/// The web's `getTileDimensions(variant, size)`.
pub(crate) fn pattern_tile_dimensions(variant: &str, size: f64) -> Option<(f64, f64)> {
    let geometry = pattern_tile_geometry()
        .map_err(|error| tracing::error!(%error, "pattern-tile"))
        .ok()?;
    let Some(tile) = geometry.variants.get(variant) else {
        tracing::error!(variant, "pattern-tile variant is not in the installed geometry");
        return None;
    };
    Some((size, size * tile.height / geometry.unit))
}

/// The web's `<PatternTile>`: a `<g>` of the variant's strokes scaled from the geometry unit to `size`.
pub(crate) fn pattern_tile_group(variant: &str, size: f64, color: &str, stroke_width: f64) -> Element {
    let geometry = match pattern_tile_geometry() {
        Ok(geometry) => geometry,
        Err(error) => {
            tracing::error!(%error, "pattern-tile");
            return rsx! {};
        }
    };
    let Some(tile) = geometry.variants.get(variant) else {
        tracing::error!(variant, "pattern-tile variant is not in the installed geometry");
        return rsx! {};
    };
    let scale = size / geometry.unit;
    let unit_stroke = if scale == 0.0 { stroke_width } else { stroke_width / scale };
    let transform = format!("scale({scale})");
    rsx! {
        g {
            g { transform: "{transform}", fill: "none", stroke: "{color}", stroke_width: "{unit_stroke}",
                for (index, element) in tile.elements.iter().enumerate() {
                    match element {
                        PatternTileElement::Path { d, opacity } => rsx! {
                            path { key: "{index}", d: "{d}", opacity: opacity.map(|value| value.to_string()) }
                        },
                        PatternTileElement::Circle { cx, cy, r, filled, opacity } => rsx! {
                            circle { key: "{index}", cx: "{cx}", cy: "{cy}", r: "{r}", fill: if *filled { color.to_string() } else { "none".to_string() }, opacity: opacity.map(|value| value.to_string()) }
                        },
                    }
                }
            }
        }
    }
}
