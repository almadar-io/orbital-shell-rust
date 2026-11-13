//! Native body of the `command-palette` pattern, mirroring the web `CommandPalette` molecule.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, DioxusPatternRenderer,
    DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `command-palette` pattern.
pub struct CommandPaletteRenderer;

impl CommandPaletteRenderer {
    const NAME: &str = "command-palette";
}

#[derive(Props, Clone, PartialEq)]
struct PaletteProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn matches_query(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut wanted = query.chars().flat_map(char::to_lowercase).peekable();
    for ch in text.chars().flat_map(char::to_lowercase) {
        if wanted.peek() == Some(&ch) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

fn command_matches(command: &Value, query: &str) -> bool {
    query.is_empty()
        || matches_query(query, &get_string_or(command, "label", ""))
        || get_string_array(command, "keywords")
            .iter()
            .any(|keyword| matches_query(query, keyword))
}

use crate::renderer::emit_event as emit;

#[allow(non_snake_case)]
fn Palette(input: PaletteProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut query = use_signal(String::new);
    let mut highlight = use_signal(|| 0usize);

    let open = get_bool_or(props, "open", false);
    let placeholder = get_string_or(
        props,
        "placeholder",
        &crate::i18n::t("commandPalette.placeholder"),
    );
    let empty_label = get_string_or(
        props,
        "emptyLabel",
        &crate::i18n::t("commandPalette.noMatches"),
    );
    let commands = get_array(props, "commands");
    let current_query = query();
    let filtered: Vec<Value> = commands
        .iter()
        .filter(|command| command_matches(command, &current_query))
        .cloned()
        .collect();
    let filtered_ids: Vec<String> = filtered
        .iter()
        .map(|c| get_string_or(c, "id", ""))
        .collect();

    let sink = view.sink;
    let on_open_change = get_string(props, "onOpenChange");
    let on_select = get_string(props, "onSelect").filter(|name| !name.is_empty());

    let close = Callback::new({
        let on_open_change = on_open_change.clone();
        move |_: ()| {
            query.set(String::new());
            highlight.set(0);
            emit(
                sink,
                &on_open_change,
                object_value([("open", Value::Bool(false))]),
            );
        }
    });
    let select_command = Callback::new({
        let commands = commands.clone();
        move |id: String| {
            let Some(command) = commands.iter().find(|c| get_string_or(c, "id", "") == id) else {
                return;
            };
            if get_bool_or(command, "disabled", false) {
                return;
            }
            emit(
                sink,
                &get_string(command, "event"),
                object_value([("commandId", Value::string(id.to_string()))]),
            );
            let payload = command
                .get("actionPayload")
                .cloned()
                .unwrap_or_else(|| object_value::<&str>([]));
            emit(sink, &get_string(command, "action"), payload);
            emit(
                sink,
                &on_select,
                object_value([("command", command.clone())]),
            );
            close.call(());
        }
    });
    let relay: EventSink =
        Callback::new(
            move |(event, payload): (String, Value)| match event.as_str() {
                "__cp:query" => {
                    query.set(get_string_or(&payload, "value", ""));
                    highlight.set(0);
                }
                "__cp:clear" => {
                    query.set(String::new());
                    highlight.set(0);
                }
                "__cp:close" => close.call(()),
                "__cp:select" => {
                    if let Some(id) = get_string(&payload, "id") {
                        select_command.call(id);
                    }
                }
                other => {
                    if let Some(index) = other
                        .strip_prefix("__cp:hover:")
                        .and_then(|i| i.parse::<usize>().ok())
                    {
                        if get_bool_or(&payload, "hovered", false) {
                            highlight.set(index);
                        }
                    }
                }
            },
        );
    let local = view.for_source(view.source_trait.clone(), relay);

    if !open {
        return rsx! {};
    }

    let current_highlight = highlight();
    let mut group_order: Vec<Option<String>> = Vec::new();
    for command in &filtered {
        let group = get_string(command, "group").filter(|g| !g.is_empty());
        if !group_order.contains(&group) {
            group_order.push(group);
        }
    }
    let group_nodes: Vec<Value> = group_order
        .iter()
        .map(|group| {
            let mut children = Vec::new();
            if let Some(group) = group {
                children.push(pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("caption")),
                        ("color", Value::string("muted")),
                        ("className", Value::string("px-3 pt-2 pb-1 block")),
                        ("content", Value::string(group.to_string())),
                    ],
                ));
            }
            for (index, command) in filtered.iter().enumerate() {
                if get_string(command, "group").filter(|g| !g.is_empty()) != *group {
                    continue;
                }
                let disabled = get_bool_or(command, "disabled", false);
                let highlighted = index == current_highlight;
                let class = cn(&[
                    "w-full flex items-center gap-3 px-3 py-2 text-start rounded-interactive",
                    "text-sm transition-colors",
                    "focus:outline-none",
                    "focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset",
                    if highlighted { "bg-muted" } else { "" },
                    if disabled { "opacity-50 cursor-not-allowed" } else { "" },
                ]);
                let mut row_children = Vec::new();
                if let Some(icon) = command.get("icon").and_then(Value::as_str).filter(|i| !i.is_empty()) {
                    row_children.push(pattern_spec(
                        "icon",
                        [
                            ("name", Value::string(icon)),
                            ("size", Value::string("sm")),
                            ("className", Value::string("flex-shrink-0")),
                        ],
                    ));
                }
                row_children.push(pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("small")),
                        ("className", Value::string("flex-1 truncate")),
                        ("content", Value::string(get_string_or(command, "label", ""))),
                    ],
                ));
                if let Some(shortcut) = get_string(command, "shortcut").filter(|s| !s.is_empty()) {
                    row_children.push(pattern_spec(
                        "badge",
                        [
                            ("label", Value::string(shortcut)),
                            ("variant", Value::string("neutral")),
                            ("size", Value::string("sm")),
                        ],
                    ));
                }
                let mut fields = vec![
                    ("className", Value::string(class)),
                    ("role", Value::string("option")),
                    ("aria-selected", Value::Bool(highlighted)),
                    ("children", array_value(row_children)),
                ];
                if !disabled {
                    fields.push(("hoverEvent", Value::string(format!("__cp:hover:{index}"))));
                    fields.push(("action", Value::string("__cp:select")));
                    fields.push((
                        "actionPayload",
                        object_value([("id", Value::string(get_string_or(command, "id", "")))]),
                    ));
                } else {
                    fields.push(("aria-disabled", Value::Bool(true)));
                }
                children.push(pattern_spec("box", fields));
            }
            pattern_spec("box", [("children", array_value(children))])
        })
        .collect();

    let search = pattern_spec(
        "input",
        [
            ("inputType", Value::string("search")),
            ("placeholder", Value::string(placeholder.to_string())),
            ("value", Value::string(current_query.to_string())),
            ("onChange", Value::string("__cp:query")),
            ("leftIcon", Value::string("search")),
            ("clearable", Value::Bool(!current_query.is_empty())),
            ("onClear", Value::string("__cp:clear")),
        ],
    );
    let body = if filtered.is_empty() {
        pattern_spec(
            "box",
            [
                ("className", Value::string("px-2 py-6 text-center")),
                (
                    "children",
                    array_value(vec![pattern_spec(
                        "typography",
                        [
                            ("variant", Value::string("body")),
                            ("color", Value::string("muted")),
                            ("content", Value::string(empty_label.to_string())),
                        ],
                    )]),
                ),
            ],
        )
    } else {
        pattern_spec(
            "box",
            [
                ("className", Value::string("max-h-80 overflow-y-auto")),
                ("role", Value::string("listbox")),
                ("aria-label", Value::string(placeholder.to_string())),
                (
                    "children",
                    array_value(vec![pattern_spec(
                        "vstack",
                        [
                            ("gap", Value::string("none")),
                            ("children", array_value(group_nodes)),
                        ],
                    )]),
                ),
            ],
        )
    };
    let content = pattern_spec(
        "box",
        [(
            "children",
            array_value(vec![pattern_spec(
                "vstack",
                [
                    ("gap", Value::string("sm")),
                    ("children", array_value(vec![search, body])),
                ],
            )]),
        )],
    );
    let mut modal_fields: Vec<(String, Value)> = dom_passthrough(props);
    modal_fields.push(("isOpen".into(), Value::Bool(true)));
    modal_fields.push(("closeEvent".into(), Value::string("__cp:close")));
    modal_fields.push(("showCloseButton".into(), Value::Bool(false)));
    modal_fields.push(("size".into(), Value::string("md")));
    if let Some(class) = get_string(props, "className") {
        modal_fields.push(("className".into(), Value::string(class)));
    }
    modal_fields.push(("children".into(), array_value(vec![content])));
    let modal = pattern_spec("modal", modal_fields);

    let count = filtered_ids.len();
    rsx! {
        div { style: "display:contents",
            onkeydown: move |event| {
                if count == 0 {
                    return;
                }
                match event.key() {
                    Key::ArrowDown => {
                        event.prevent_default();
                        let prev = highlight.peek().to_owned();
                        highlight.set(if prev + 1 < count { prev + 1 } else { 0 });
                    }
                    Key::ArrowUp => {
                        event.prevent_default();
                        let prev = highlight.peek().to_owned();
                        highlight.set(if prev > 0 { prev - 1 } else { count - 1 });
                    }
                    Key::Enter => {
                        event.prevent_default();
                        if let Some(id) = filtered_ids.get(*highlight.peek()) {
                            select_command.call(id.clone());
                        }
                    }
                    _ => {}
                }
            },
            {registry.render_spec(&modal, None, &local)}
        }
    }
}

impl DioxusPatternRenderer for CommandPaletteRenderer {
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
        rsx! { Palette { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
