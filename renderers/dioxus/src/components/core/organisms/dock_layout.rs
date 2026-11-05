use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    array_value, cn, layout_box_spec, pattern_spec, DioxusPatternRenderer, DioxusView, DockCommand,
    DockContext, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool, get_bool_or, get_number_or, get_string};
use orbital_core::Value;
use std::cell::Cell;
use std::rc::Rc;

pub struct DockLayoutRenderer;

const BOX: &str = "bg-transparent rounded-none shadow-none";

#[derive(Clone, Copy, PartialEq)]
enum OpenDrawer {
    Left,
    Secondary,
    Bottom,
}

fn is_compact(props: &Value, viewport_width: Option<f64>) -> bool {
    get_bool_or(
        props,
        "compact",
        viewport_width.is_some_and(|width| width < 1024.0),
    )
}

fn region_text(region: &str, collapsed: bool) -> &'static str {
    match (region, collapsed) {
        ("sidebar", false) => "Collapse side panel",
        ("sidebar", true) => "Expand side panel",
        ("secondary-sidebar", false) => "Collapse right panel",
        ("secondary-sidebar", true) => "Expand right panel",
        ("bottom-panel", false) => "Collapse bottom panel",
        _ => "Expand bottom panel",
    }
}

fn region_icon(region: &str, collapsed: bool) -> &'static str {
    match (region, collapsed) {
        ("sidebar", false) => "panel-left-close",
        ("sidebar", true) => "panel-left-open",
        ("secondary-sidebar", false) => "panel-right-close",
        ("secondary-sidebar", true) => "panel-right-open",
        ("bottom-panel", false) => "panel-bottom-close",
        _ => "panel-bottom-open",
    }
}

fn region_tooltip(region: &str) -> &'static str {
    match region {
        "sidebar" => "right",
        "secondary-sidebar" => "left",
        _ => "top",
    }
}

fn node_children(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.iter().cloned().collect(),
        other => vec![other.clone()],
    }
}

fn stack(row: bool, class: &str, children: Element) -> Element {
    let direction = if row { "flex-row" } else { "flex-col" };
    let class = cn(&[
        "flex",
        direction,
        "gap-0 items-stretch justify-start",
        class,
    ]);
    rsx! { div { class: "{class}", {children} } }
}

fn region(class: &str, style: &str, children: Element) -> Element {
    let class = cn(&[BOX, class]);
    rsx! { div { class: "{class}", style: "{style}", {children} } }
}

fn split_pane(horizontal: bool, ratio: f64, class: &str, left: Element, right: Element) -> Element {
    let root = cn(&[
        "flex w-full h-full",
        if horizontal { "flex-row" } else { "flex-col" },
        class,
    ]);
    let size = if horizontal { "width" } else { "height" };
    let handle = if horizontal {
        "flex-shrink-0 bg-border transition-colors touch-none w-1 cursor-col-resize hover:w-1.5 hover:bg-muted-foreground"
    } else {
        "flex-shrink-0 bg-border transition-colors touch-none h-1 cursor-row-resize hover:h-1.5 hover:bg-muted-foreground"
    };
    rsx! {
        div { class: "{root}",
            div { class: "flex flex-col overflow-auto", style: "{size}:{ratio}%;flex-shrink:0", {left} }
            div { class: "{handle}" }
            div { class: "flex flex-col flex-1 min-h-0 min-w-0 overflow-auto", {right} }
        }
    }
}

fn toggle_spec(region: &str, collapsed: bool, action: &str) -> Value {
    pattern_spec(
        "icon-button",
        [
            ("icon", Value::string(region_icon(region, collapsed))),
            ("label", Value::string(region_text(region, collapsed))),
            ("tooltipPosition", Value::string(region_tooltip(region))),
            ("aria-expanded", Value::Bool(!collapsed)),
            (
                "data-testid",
                Value::string(format!("dock-collapse-{region}")),
            ),
            ("className", Value::string("w-7 h-7")),
            ("action", Value::string(action)),
        ],
    )
}

fn top_button(icon: &str, label: &str, test_id: &str, action: &str) -> Value {
    pattern_spec(
        "icon-button",
        [
            ("icon", Value::string(icon)),
            ("label", Value::string(label)),
            ("tooltipPosition", Value::string("bottom")),
            ("data-testid", Value::string(test_id)),
            ("className", Value::string("w-10 h-10")),
            ("action", Value::string(action)),
        ],
    )
}

fn drawer_spec(
    open: bool,
    position: &str,
    title: String,
    close_label: Option<&String>,
    children: Vec<Value>,
) -> Value {
    let mut fields = vec![
        ("isOpen", Value::Bool(open)),
        ("position", Value::string(position)),
        ("width", Value::string("md")),
        ("title", Value::string(title)),
        ("closeEvent", Value::string("dock:close")),
        ("children", array_value(children)),
    ];
    if let Some(label) = close_label {
        fields.push(("closeButtonLabel", Value::string(label.clone())));
    }
    pattern_spec("drawer", fields)
}

#[derive(Props, Clone, PartialEq)]
struct DockLayoutProps {
    props: Value,
    entity: Option<Value>,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn DockLayoutComponent(input: DockLayoutProps) -> Element {
    let mut open = use_signal(|| None::<OpenDrawer>);
    let mut left_panel = use_signal(|| false);
    let mut sidebar_local = use_signal(|| false);
    let mut bottom_local = use_signal(|| false);
    let mut secondary_local = use_signal(|| false);
    let props = &input.props;
    let sidebar_ratio = use_signal(|| get_number_or(props, "sidebarWidth", 20.0));
    let secondary_ratio = use_signal(|| get_number_or(props, "secondarySidebarWidth", 25.0));
    let bottom_ratio = use_signal(|| get_number_or(props, "bottomPanelHeight", 30.0));
    let has_rail = use_hook(|| Rc::new(Cell::new(false)));
    let compact = is_compact(props, input.view.viewport_width);
    if !compact {
        if open.peek().is_some() {
            open.set(None);
        }
        if *left_panel.peek() {
            left_panel.set(false);
        }
    }
    let present = |name: &str| {
        props
            .as_object()
            .and_then(|fields| fields.get(name))
            .filter(|value| !value.is_null() && value.as_str() != Some(""))
    };
    let rail = present("rail");
    let sidebar = present("sidebar");
    let main = present("main");
    let bottom = present("bottomPanel");
    let secondary = present("secondarySidebar");
    let status = present("statusBar");
    let actions = present("topBarActions");
    has_rail.set(rail.is_some());

    let outer = input.view.sink;
    let sink = {
        let has_rail = has_rail.clone();
        use_callback(
            move |(event, payload): (String, Value)| match event.as_str() {
                "dock:close" => open.set(None),
                "dock:back" => left_panel.set(false),
                "dock:open:left" => {
                    left_panel.set(!has_rail.get());
                    open.set(Some(OpenDrawer::Left));
                }
                "dock:open:secondary" => open.set(Some(OpenDrawer::Secondary)),
                "dock:open:bottom" => open.set(Some(OpenDrawer::Bottom)),
                "dock:toggle:sidebar" => { let next = !*sidebar_local.peek(); sidebar_local.set(next); }
                "dock:toggle:bottom-panel" => { let next = !*bottom_local.peek(); bottom_local.set(next); }
                "dock:toggle:secondary-sidebar" => { let next = !*secondary_local.peek(); secondary_local.set(next); }
                _ => {
                    if let Some(outer) = outer {
                        outer.call((event, payload));
                    }
                }
            },
        )
    };
    let command = use_callback(move |event: DockCommand| match event {
        DockCommand::ShowPanel => left_panel.set(true),
        DockCommand::CloseMenu => open.set(None),
    });
    let mut base = input.view.as_view();
    base.dock = Some(DockContext { compact, command });
    let view = base.for_source(base.source_trait.clone(), sink);
    let registry = &input.registry;
    let node = |value: Option<&Value>| value.map(|value| view.render_node(registry, value));
    let chrome = |spec: &Value| registry.render_spec(spec, None, &view);
    let class_of = |name: &str| get_string(props, name).unwrap_or_default();
    let class = class_of("className");
    let main_class = class_of("mainClassName");
    let status_class = class_of("statusBarClassName");
    let root_class = cn(&["w-full h-full overflow-hidden", &class]);
    let status_node = status.map(|value| {
        let class = cn(&[
            "flex-shrink-0 border-t border-border bg-background",
            &status_class,
        ]);
        region(&class, "", view.render_node(registry, value))
    });
    let main_node = node(main);

    if compact {
        let open_drawer = *open.read();
        let show_panel = *left_panel.read();
        let text = |name: &str, default: &str| get_string(props, name).unwrap_or(default.into());
        let menu_label = text("menuLabel", "Menu");
        let secondary_label = text("secondarySidebarLabel", "Expand right panel");
        let bottom_label = text("bottomPanelLabel", "Expand bottom panel");
        let close_label = get_string(props, "closeDrawerLabel");
        let top_class =
            "flex flex-shrink-0 items-center gap-1 px-2 py-1 border-b border-border bg-surface";
        let top_bar = (rail.is_some()
            || sidebar.is_some()
            || secondary.is_some()
            || bottom.is_some()
            || actions.is_some())
        .then(|| {
            let top_class = cn(&[BOX, top_class]);
            let flex = cn(&[BOX, "flex-1"]);
            rsx! {
                div { class: "{top_class}", "data-testid": "dock-top-bar",
                    if rail.is_some() || sidebar.is_some() {
                        {chrome(&top_button("menu", &text("openMenuLabel", "Menu"), "dock-menu", "dock:open:left"))}
                    }
                    div { class: "{flex}" }
                    if bottom.is_some() {
                        {chrome(&top_button(
                            &get_string(props, "bottomPanelIcon").unwrap_or("panel-bottom-open".into()),
                            &bottom_label,
                            "dock-open-bottom-panel",
                            "dock:open:bottom",
                        ))}
                    }
                    if secondary.is_some() {
                        {chrome(&top_button(
                            &get_string(props, "secondarySidebarIcon").unwrap_or("panel-right-open".into()),
                            &secondary_label,
                            "dock-open-secondary-sidebar",
                            "dock:open:secondary",
                        ))}
                    }
                    {node(actions)}
                }
            }
        });
        let left_children = if !show_panel && rail.is_some() {
            rail.map(node_children).unwrap_or_default()
        } else {
            let mut parts = Vec::new();
            if rail.is_some() {
                let back = pattern_spec(
                    "icon-button",
                    [
                        ("icon", Value::string("arrow-left")),
                        (
                            "label",
                            Value::string(text("backToMenuLabel", "Back to the menu")),
                        ),
                        ("tooltipPosition", Value::string("bottom")),
                        ("data-testid", Value::string("dock-drawer-back")),
                        ("className", Value::string("w-10 h-10")),
                        ("action", Value::string("dock:back")),
                    ],
                );
                parts.push(pattern_spec(
                    "hstack",
                    [
                        ("gap", Value::string("xs")),
                        (
                            "className",
                            Value::string(
                                "flex-shrink-0 items-center px-2 py-1 border-b border-border",
                            ),
                        ),
                        ("children", array_value(vec![back])),
                    ],
                ));
            }
            parts.push(layout_box_spec(
                "flex-1 min-h-0 overflow-auto",
                None,
                Vec::new(),
                sidebar.map(node_children).unwrap_or_default(),
            ));
            vec![pattern_spec(
                "vstack",
                [
                    ("gap", Value::string("none")),
                    ("className", Value::string("h-full min-h-0")),
                    ("children", array_value(parts)),
                ],
            )]
        };
        let left_drawer = (rail.is_some() || sidebar.is_some()).then(|| {
            let open_left = open_drawer == Some(OpenDrawer::Left);
            chrome(&drawer_spec(
                open_left,
                "left",
                menu_label.clone(),
                close_label.as_ref(),
                if open_left { left_children } else { Vec::new() },
            ))
        });
        let secondary_drawer = secondary.map(|value| {
            let open_it = open_drawer == Some(OpenDrawer::Secondary);
            chrome(&drawer_spec(
                open_it,
                "right",
                secondary_label.clone(),
                close_label.as_ref(),
                if open_it {
                    node_children(value)
                } else {
                    Vec::new()
                },
            ))
        });
        let bottom_drawer = bottom.map(|value| {
            let open_it = open_drawer == Some(OpenDrawer::Bottom);
            chrome(&drawer_spec(
                open_it,
                "right",
                bottom_label.clone(),
                close_label.as_ref(),
                if open_it {
                    node_children(value)
                } else {
                    Vec::new()
                },
            ))
        });
        let main_region = cn(&[BOX, "flex-1 min-h-0 min-w-0 overflow-auto", &main_class]);
        let root = cn(&[
            "flex flex-col gap-0 items-stretch justify-start",
            &root_class,
        ]);
        return rsx! {
            div { class: "{root}", ..shared_root_attrs(props),
                {top_bar}
                div { class: "{main_region}", {main_node} }
                {status_node}
                {left_drawer}
                {secondary_drawer}
                {bottom_drawer}
            }
        };
    }

    let collapsed = |name: &str, local: bool| get_bool(props, name).unwrap_or(local);
    let sidebar_collapsed = collapsed("sidebarCollapsed", *sidebar_local.read());
    let bottom_collapsed = collapsed("bottomPanelCollapsed", *bottom_local.read());
    let secondary_collapsed = collapsed("secondarySidebarCollapsed", *secondary_local.read());
    let show_sidebar = sidebar.is_some() && !sidebar_collapsed;
    let show_bottom = bottom.is_some() && !bottom_collapsed;
    let show_secondary = secondary.is_some() && !secondary_collapsed;

    let gutter = |name: &str, is_collapsed: bool, edge: &str| {
        let class = cn(&[
            "flex-shrink-0 w-8 flex flex-col items-center pt-1 bg-surface",
            if edge == "l" {
                "border-l border-border"
            } else {
                "border-r border-border"
            },
        ]);
        let action = format!("dock:toggle:{name}");
        region(
            &class,
            "",
            chrome(&toggle_spec(name, is_collapsed, &action)),
        )
    };
    let main_region = region(
        &cn(&["flex-1 min-w-0 min-h-0 overflow-auto", &main_class]),
        "",
        main_node.clone().unwrap_or(rsx! {}),
    );
    let center_row = if show_secondary {
        let ratio = *secondary_ratio.read();
        let panel = region(
            &cn(&[
                "flex-1 min-w-0 h-full overflow-auto",
                &class_of("secondarySidebarClassName"),
            ]),
            "",
            node(secondary).unwrap_or(rsx! {}),
        );
        split_pane(
            true,
            100.0 - ratio,
            "flex-1 min-h-0 min-w-0",
            main_region,
            stack(
                true,
                "h-full min-w-0",
                rsx! { {gutter("secondary-sidebar", false, "l")} {panel} },
            ),
        )
    } else {
        let closed_gutter = secondary
            .is_some()
            .then(|| gutter("secondary-sidebar", true, "l"));
        stack(
            true,
            "flex-1 min-h-0 min-w-0",
            rsx! { {main_region} {closed_gutter} },
        )
    };
    let sidebar_and_center = if show_sidebar {
        let ratio = *sidebar_ratio.read();
        let panel = region(
            &cn(&["h-full overflow-auto", &class_of("sidebarClassName")]),
            "",
            node(sidebar).unwrap_or(rsx! {}),
        );
        split_pane(
            true,
            ratio,
            "flex-1 min-h-0 min-w-0",
            panel,
            stack(
                true,
                "h-full min-w-0",
                rsx! { {gutter("sidebar", false, "r")} {center_row} },
            ),
        )
    } else if sidebar.is_some() {
        stack(
            true,
            "flex-1 min-h-0 min-w-0",
            rsx! { {gutter("sidebar", true, "r")} {center_row} },
        )
    } else {
        center_row
    };
    let bottom_gutter = bottom.is_some().then(|| {
        stack(
            true,
            "flex-shrink-0 h-8 items-center justify-end px-1 border-t border-border bg-surface",
            chrome(&toggle_spec(
                "bottom-panel",
                bottom_collapsed,
                "dock:toggle:bottom-panel",
            )),
        )
    });
    let rail_node = rail.map(|value| {
        let class = cn(&[
            "flex-shrink-0 min-h-0 overflow-y-auto overflow-x-hidden border-r border-border",
            &class_of("railClassName"),
        ]);
        let style = props
            .get("railWidth")
            .and_then(Value::as_number)
            .map(|width| format!("width:{width}px"))
            .unwrap_or_default();
        region(&class, &style, view.render_node(registry, value))
    });
    let body = stack(
        true,
        "flex-1 min-h-0 min-w-0",
        rsx! { {rail_node} {sidebar_and_center} },
    );
    let body_plus_bottom = if show_bottom {
        let ratio = *bottom_ratio.read();
        let panel = region(
            &cn(&[
                "flex-1 min-h-0 overflow-auto",
                &class_of("bottomPanelClassName"),
            ]),
            "",
            node(bottom).unwrap_or(rsx! {}),
        );
        split_pane(
            false,
            100.0 - ratio,
            "flex-1 min-h-0",
            body,
            stack(false, "h-full", rsx! { {bottom_gutter} {panel} }),
        )
    } else if bottom.is_some() {
        stack(false, "flex-1 min-h-0", rsx! { {body} {bottom_gutter} })
    } else {
        body
    };
    let root = cn(&[
        "flex flex-col gap-0 items-stretch justify-start",
        &root_class,
    ]);
    rsx! {
        div { class: "{root}", ..shared_root_attrs(props),
            {body_plus_bottom}
            {status_node}
        }
    }
}

impl DioxusPatternRenderer for DockLayoutRenderer {
    fn pattern_name(&self) -> &str {
        "dock-layout"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { DockLayoutComponent { props: props.clone(), entity: entity.cloned(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[cfg(test)]
mod tests {
    use super::is_compact;

    #[test]
    fn explicit_compact_overrides_logical_viewport_threshold() {
        let automatic: orbital_core::Value =
            serde_json::from_str(r#"{"type":"dock-layout"}"#).unwrap();
        let desktop: orbital_core::Value =
            serde_json::from_str(r#"{"type":"dock-layout","compact":false}"#).unwrap();
        let mobile: orbital_core::Value =
            serde_json::from_str(r#"{"type":"dock-layout","compact":true}"#).unwrap();
        assert!(is_compact(&automatic, Some(1023.0)));
        assert!(!is_compact(&automatic, Some(1024.0)));
        assert!(!is_compact(&desktop, Some(320.0)));
        assert!(is_compact(&mobile, Some(1600.0)));
    }
}
