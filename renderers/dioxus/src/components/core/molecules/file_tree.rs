//! Native body of the `file-tree` pattern, mirroring the web `FileTree` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    object_value, pattern_spec, typography_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::{HashMap, HashSet};

/// Dioxus renderer for the `file-tree` pattern.
pub struct FileTreeRenderer;

impl FileTreeRenderer {
    const NAME: &str = "file-tree";
}

#[derive(Props, Clone, PartialEq)]
struct TreeProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

struct Node {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<Node>,
    note: Option<String>,
    expandable: bool,
    size: Option<f64>,
}

struct Item {
    id: String,
    label: String,
    parent_id: Option<String>,
    icon: Option<String>,
}

fn parse_node(value: &Value) -> Node {
    Node {
        name: get_string_or(value, "name", ""),
        path: get_string_or(value, "path", ""),
        is_dir: get_string(value, "type").as_deref() == Some("dir"),
        children: get_array(value, "children")
            .iter()
            .map(parse_node)
            .collect(),
        note: get_string(value, "note"),
        expandable: get_bool_or(value, "expandable", false),
        size: get_number(value, "size"),
    }
}

fn parse_item(value: &Value) -> Item {
    Item {
        id: get_string_or(value, "id", ""),
        label: get_string_or(value, "label", ""),
        parent_id: get_string(value, "parentId").filter(|id| !id.is_empty()),
        icon: get_string(value, "icon").filter(|icon| !icon.is_empty()),
    }
}

fn file_icon(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "ts" | "tsx" | "js" | "jsx" | "py" | "html" => "file-code",
        "json" | "orb" => "file-json",
        "css" | "scss" | "md" => "file-text",
        _ => "file",
    }
}

fn format_size(size: f64) -> String {
    if size < 1024.0 {
        format!("{}B", js_number(size))
    } else {
        format!("{}KB", js_number((size / 1024.0 + 0.5).floor()))
    }
}

fn js_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn icon_spec(name: &str, class: &str) -> Value {
    pattern_spec(
        "icon",
        [
            ("name", Value::string(name)),
            ("size", Value::string("xs")),
            ("className", Value::string(class)),
        ],
    )
}

fn emit(sink: Option<EventSink>, event: &Option<String>, field: &str, value: &str) {
    if let (Some(sink), Some(event)) = (sink, event) {
        sink.call((event.clone(), object_value([(field, Value::string(value))])));
    }
}

#[derive(Clone)]
struct Shared {
    sink: Option<EventSink>,
    active: Signal<Option<String>>,
    overrides: Signal<HashMap<String, bool>>,
    asked: Signal<HashSet<String>>,
    on_file_select: Option<String>,
    on_node_expand: Option<String>,
    on_node_select: Option<String>,
    on_node_action: Option<String>,
    node_action_icon: String,
    node_action_label: String,
    indent: f64,
    selected: Option<String>,
    look_nav: bool,
    default_active: Option<String>,
}

impl Shared {
    fn active_key(&self) -> Option<String> {
        self.active
            .read()
            .clone()
            .or_else(|| self.default_active.clone())
    }
}

fn set_override(mut overrides: Signal<HashMap<String, bool>>, key: &str, value: bool) {
    let mut next = overrides.peek().clone();
    next.insert(key.to_string(), value);
    overrides.set(next);
}

fn tree_node(
    node: &Node,
    depth: usize,
    s: &Shared,
    revealed: &HashSet<String>,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let has_derived = !node.is_dir && (!node.children.is_empty() || node.expandable);
    let expandable_row = node.is_dir || has_derived;
    let is_revealed = revealed.contains(&node.path);
    let default = is_revealed || (node.is_dir && depth < 1);
    let expanded = s
        .overrides
        .read()
        .get(&node.path)
        .copied()
        .unwrap_or(default);
    let is_selected = s.selected.as_deref() == Some(node.path.as_str());
    let class = format!(
        "{BOX_ATOM_BASE} flex items-center gap-1.5 py-0.5 px-2 cursor-pointer rounded-interactive transition-colors {}",
        if is_selected { "bg-primary text-primary-foreground" } else { "hover:bg-muted" }
    );
    let style = format!(
        "padding-left:{}px",
        js_number(depth as f64 * s.indent + 8.0)
    );
    let tabindex = if s.active_key().as_deref() == Some(node.path.as_str()) {
        "0"
    } else {
        "-1"
    };
    let aria_expanded = expandable_row.then(|| expanded.to_string());

    let path = node.path.clone();
    let is_dir = node.is_dir;
    let expandable = node.expandable;
    let sink = s.sink;
    let overrides = s.overrides;
    let asked = s.asked;
    let on_file_select = s.on_file_select.clone();
    let on_node_expand = s.on_node_expand.clone();
    let toggle_derived = {
        let path = path.clone();
        let on_node_expand = on_node_expand.clone();
        Callback::new(move |_: ()| {
            if !expanded && expandable && !asked.peek().contains(&path) {
                let mut asked = asked;
                let mut next = asked.peek().clone();
                next.insert(path.clone());
                asked.set(next);
                emit(sink, &on_node_expand, "path", &path);
            }
            set_override(overrides, &path, !expanded);
        })
    };
    let toggle = {
        let path = path.clone();
        Callback::new(move |_: ()| {
            if is_dir {
                set_override(overrides, &path, !expanded);
            } else {
                toggle_derived.call(());
            }
        })
    };
    let activate = {
        let path = path.clone();
        Callback::new(move |_: ()| {
            if is_dir {
                set_override(overrides, &path, !expanded);
            } else {
                emit(sink, &on_file_select, "path", &path);
            }
        })
    };
    let focus_path = path.clone();
    let mut active = s.active;

    let icon_name = if node.is_dir {
        if expanded {
            "folder-open"
        } else {
            "folder"
        }
    } else {
        file_icon(&node.name)
    };
    let icon_class = if is_selected {
        "text-inherit"
    } else if node.is_dir {
        "text-[var(--color-warning)]"
    } else {
        "text-[var(--color-muted-foreground)]"
    };
    let chevron = if expanded {
        "chevron-down"
    } else {
        "chevron-right"
    };
    let name_class = format!(
        "truncate font-mono text-xs !text-inherit {}",
        if is_selected { "font-semibold" } else { "" }
    );
    let name = typography_spec("caption", None, name_class.trim(), &node.name);
    let note_class = format!(
        "text-[9px] flex-shrink-0 {}",
        if is_selected {
            "!text-inherit opacity-80"
        } else {
            "text-[var(--color-muted-foreground)]"
        }
    );
    let note = node
        .note
        .as_ref()
        .map(|note| typography_spec("caption", None, &note_class, note));
    let size = node.size.filter(|_| !node.is_dir).map(|size| {
        let class = format!(
            "text-[9px] ml-auto flex-shrink-0 {}",
            if is_selected {
                "!text-inherit opacity-80"
            } else {
                "text-[var(--color-muted-foreground)]"
            }
        );
        typography_spec("caption", None, &class, &format_size(size))
    });
    let dir_chevron = icon_spec(
        chevron,
        "text-[var(--color-muted-foreground)] flex-shrink-0",
    );
    let derived_chevron = icon_spec(chevron, "text-[var(--color-muted-foreground)]");
    let row_icon = icon_spec(icon_name, icon_class);
    let toggle_label = if expanded { "Collapse" } else { "Expand" };
    let show_children = expandable_row && expanded && !node.children.is_empty();

    rsx! {
        div { key: "{node.path}", class: "{class}", style: "{style}", role: "treeitem", tabindex: "{tabindex}",
            aria_level: "{depth + 1}", aria_selected: "{is_selected}", aria_expanded: aria_expanded,
            onclick: move |_| activate.call(()),
            onfocus: move |_| active.set(Some(focus_path.clone())),
            onkeydown: move |event| {
                match event.key() {
                    Key::Enter => { event.prevent_default(); activate.call(()); }
                    Key::Character(c) if c == " " => { event.prevent_default(); activate.call(()); }
                    Key::ArrowRight if expandable_row && !expanded => { event.prevent_default(); toggle.call(()); }
                    Key::ArrowLeft if expandable_row && expanded => { event.prevent_default(); toggle.call(()); }
                    _ => {}
                }
            },
            if node.is_dir {
                {registry.render_spec(&dir_chevron, None, view)}
            } else if has_derived {
                div { class: "{BOX_ATOM_BASE} flex-shrink-0 flex items-center", role: "button", tabindex: "-1",
                    aria_expanded: "{expanded}", aria_label: "{toggle_label}",
                    onclick: move |event| { event.stop_propagation(); toggle_derived.call(()); },
                    {registry.render_spec(&derived_chevron, None, view)}
                }
            } else {
                div { class: "{BOX_ATOM_BASE}", style: "width:12px;flex-shrink:0" }
            }
            {registry.render_spec(&row_icon, None, view)}
            {registry.render_spec(&name, None, view)}
            if let Some(note) = note.as_ref() { {registry.render_spec(note, None, view)} }
            if let Some(size) = size.as_ref() { {registry.render_spec(size, None, view)} }
        }
        if show_children {
            div { class: "{BOX_ATOM_BASE}", role: "group",
                for child in node.children.iter() {
                    {tree_node(child, depth + 1, s, revealed, registry, view)}
                }
            }
        }
    }
}

struct Flat<'a> {
    children: HashMap<&'a str, Vec<&'a Item>>,
    selected_ancestors: HashSet<String>,
    listed: HashSet<String>,
}

fn flat_node(
    item: &Item,
    depth: usize,
    s: &Shared,
    flat: &Flat<'_>,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let children = flat.children.get(item.id.as_str());
    let has_children = children.map(|c| !c.is_empty()).unwrap_or(false);
    let default =
        depth < 1 || flat.selected_ancestors.contains(&item.id) || flat.listed.contains(&item.id);
    let expanded = has_children && s.overrides.read().get(&item.id).copied().unwrap_or(default);
    let is_selected = s.selected.as_deref() == Some(item.id.as_str());
    let nav = s.look_nav;
    let class = format!(
        "{BOX_ATOM_BASE} group/treerow flex items-center gap-1.5 px-2 cursor-pointer rounded-interactive transition-colors {} {}",
        if nav { "py-1" } else { "py-0.5" },
        if is_selected {
            "bg-primary text-primary-foreground"
        } else if nav {
            "text-foreground hover:bg-muted"
        } else {
            "hover:bg-muted"
        }
    );
    let style = format!(
        "padding-left:{}px",
        js_number(depth as f64 * s.indent + 8.0)
    );
    let active_here = s.active_key().as_deref() == Some(item.id.as_str());
    let tabindex = if active_here { "0" } else { "-1" };
    let aria_expanded = has_children.then(|| expanded.to_string());

    let id = item.id.clone();
    let sink = s.sink;
    let overrides = s.overrides;
    let on_node_select = s.on_node_select.clone();
    let on_node_action = s.on_node_action.clone();
    let toggle = {
        let id = id.clone();
        Callback::new(move |_: ()| set_override(overrides, &id, !expanded))
    };
    let activate = {
        let id = id.clone();
        Callback::new(move |_: ()| {
            if has_children && on_node_select.is_none() {
                set_override(overrides, &id, !expanded);
            }
            emit(sink, &on_node_select, "id", &id);
        })
    };
    let action_id = id.clone();
    let focus_id = id.clone();
    let mut active = s.active;

    let chevron = if expanded {
        "chevron-down"
    } else {
        "chevron-right"
    };
    let chevron_spec = icon_spec(
        chevron,
        if is_selected {
            "text-inherit"
        } else {
            "text-[var(--color-muted-foreground)]"
        },
    );
    let icon_name = match &item.icon {
        Some(icon) => icon.as_str(),
        None if nav => "file-text",
        None if has_children => {
            if expanded {
                "folder-open"
            } else {
                "folder"
            }
        }
        None => "file",
    };
    let icon_class = if is_selected {
        "text-inherit"
    } else if nav || !has_children {
        "text-[var(--color-muted-foreground)]"
    } else {
        "text-[var(--color-warning)]"
    };
    let row_icon = icon_spec(icon_name, icon_class);
    let label_class = format!(
        "truncate {} !text-inherit {}",
        if nav { "text-sm" } else { "font-mono text-xs" },
        if is_selected { "font-semibold" } else { "" }
    );
    let label = typography_spec("caption", None, label_class.trim(), &item.label);
    let action_spec = s
        .on_node_action
        .as_ref()
        .map(|_| icon_spec(&s.node_action_icon, "text-inherit"));
    let action_class = format!(
        "{BOX_ATOM_BASE} ml-auto flex-shrink-0 rounded-interactive p-0.5 opacity-0 group-hover/treerow:opacity-100 group-focus-within/treerow:opacity-100 transition-opacity {}",
        if is_selected { "hover:bg-primary-foreground/20" } else { "hover:bg-border" }
    );
    let action_tab = if active_here { "0" } else { "-1" };
    let action_label = s.node_action_label.clone();
    let toggle_label = if expanded { "Collapse" } else { "Expand" };

    rsx! {
        div { key: "{item.id}", class: "{class}", style: "{style}", role: "treeitem", tabindex: "{tabindex}",
            aria_level: "{depth + 1}", aria_selected: "{is_selected}", aria_expanded: aria_expanded,
            onclick: move |_| activate.call(()),
            onfocus: move |_| active.set(Some(focus_id.clone())),
            onkeydown: move |event| {
                match event.key() {
                    Key::Enter => { event.prevent_default(); activate.call(()); }
                    Key::Character(c) if c == " " => { event.prevent_default(); activate.call(()); }
                    Key::ArrowRight if has_children && !expanded => { event.prevent_default(); toggle.call(()); }
                    Key::ArrowLeft if has_children && expanded => { event.prevent_default(); toggle.call(()); }
                    _ => {}
                }
            },
            if has_children {
                div { class: "{BOX_ATOM_BASE} flex items-center flex-shrink-0", role: "button", tabindex: "-1", aria_label: "{toggle_label}",
                    onclick: move |event| { event.stop_propagation(); toggle.call(()); },
                    {registry.render_spec(&chevron_spec, None, view)}
                }
            } else {
                div { class: "{BOX_ATOM_BASE}", style: "width:12px;flex-shrink:0" }
            }
            {registry.render_spec(&row_icon, None, view)}
            {registry.render_spec(&label, None, view)}
            if let Some(action_spec) = action_spec.as_ref() {
                div { class: "{action_class}", role: "button", tabindex: "{action_tab}", aria_label: "{action_label}", title: "{action_label}",
                    onclick: move |event| { event.stop_propagation(); emit(sink, &on_node_action, "id", &action_id); },
                    {registry.render_spec(action_spec, None, view)}
                }
            }
        }
        if has_children && expanded {
            div { class: "{BOX_ATOM_BASE}", role: "group",
                for child in children.into_iter().flatten() {
                    {flat_node(child, depth + 1, s, flat, registry, view)}
                }
            }
        }
    }
}

#[allow(non_snake_case)]
fn Tree(input: TreeProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let active = use_signal(|| None::<String>);
    let overrides = use_signal(HashMap::<String, bool>::new);
    let asked = use_signal(HashSet::<String>::new);
    let non_empty = |key: &str| get_string(props, key).filter(|name| !name.is_empty());
    let selected_id = non_empty("selectedId");
    let items_value = props.get("items").and_then(Value::as_array);
    let shared = Shared {
        sink: view.sink,
        active,
        overrides,
        asked,
        on_file_select: non_empty("onFileSelect"),
        on_node_expand: non_empty("onNodeExpand"),
        on_node_select: non_empty("onNodeSelect"),
        on_node_action: non_empty("onNodeAction"),
        node_action_icon: get_string_or(props, "nodeActionIcon", "plus"),
        node_action_label: get_string_or(props, "nodeActionLabel", "Node action"),
        indent: get_number_or(props, "indent", 16.0),
        selected: if items_value.is_some() {
            selected_id.clone()
        } else {
            non_empty("selectedPath")
        },
        look_nav: get_string_or(props, "look", "files") == "nav",
        default_active: None,
    };
    let listed: HashSet<String> = get_string_array(props, "expandedPaths")
        .into_iter()
        .collect();
    let class = format!(
        "py-1 overflow-y-auto {}",
        get_string_or(props, "className", "")
    );
    let attrs: Vec<Attribute> = shared_root_attrs(props)
        .into_iter()
        .filter(|attr| attr.name != "role")
        .collect();

    if let Some(items_value) = items_value {
        let items: Vec<Item> = items_value.iter().map(parse_item).collect();
        if items.is_empty() {
            return rsx! {};
        }
        let ids: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
        let mut children: HashMap<&str, Vec<&Item>> = HashMap::new();
        let mut roots: Vec<&Item> = Vec::new();
        for item in &items {
            match item
                .parent_id
                .as_deref()
                .filter(|parent| ids.contains(parent))
            {
                Some(parent) => children.entry(parent).or_default().push(item),
                None => roots.push(item),
            }
        }
        let by_id: HashMap<&str, &Item> =
            items.iter().map(|item| (item.id.as_str(), item)).collect();
        let mut selected_ancestors = HashSet::new();
        if let Some(selected) = selected_id.as_deref() {
            let mut current = by_id
                .get(selected)
                .and_then(|item| item.parent_id.as_deref());
            while let Some(parent) = current {
                if !by_id.contains_key(parent) || !selected_ancestors.insert(parent.to_string()) {
                    break;
                }
                current = by_id.get(parent).and_then(|item| item.parent_id.as_deref());
            }
        }
        let flat = Flat {
            children,
            selected_ancestors,
            listed,
        };
        let mut shared = shared;
        shared.default_active = roots.first().map(|item| item.id.clone());
        let shared_for_default = shared;
        return rsx! {
            div { class: "{BOX_ATOM_BASE} {class}", role: "tree", ..attrs,
                for item in roots.iter() {
                    {flat_node(item, 0, &shared_for_default, &flat, registry, &view)}
                }
            }
        };
    }

    let nodes: Vec<Node> = get_array(props, "tree").iter().map(parse_node).collect();
    if nodes.is_empty() {
        return rsx! {};
    }
    let mut shared = shared;
    shared.default_active = nodes.first().map(|node| node.path.clone());
    rsx! {
        div { class: "{BOX_ATOM_BASE} {class}", role: "tree", ..attrs,
            for node in nodes.iter() {
                {tree_node(node, 0, &shared, &listed, registry, &view)}
            }
        }
    }
}

impl DioxusPatternRenderer for FileTreeRenderer {
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
        rsx! { Tree { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
