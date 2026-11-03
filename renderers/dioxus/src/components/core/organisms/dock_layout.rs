use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    DioxusPatternRenderer, DioxusView, DockCommand, DockContext, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_number_or, get_string, merge_class_name};
use orbital_core::Value;

pub struct DockLayoutRenderer;

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
    let compact = is_compact(&input.props, input.view.viewport_width);
    if !compact {
        if open.peek().is_some() {
            open.set(None);
        }
        if *left_panel.peek() {
            left_panel.set(false);
        }
    }
    let command = Callback::new(move |event: DockCommand| match event {
        DockCommand::ShowPanel => left_panel.set(true),
        DockCommand::CloseMenu => open.set(None),
    });
    let mut view = input.view.as_view();
    view.dock = Some(DockContext { compact, command });
    let registry = &input.registry;
    let entity = input.entity.as_ref();
    let get = |name: &str| input.props.as_object().and_then(|props| props.get(name));
    let rail = get("rail");
    let sidebar = get("sidebar");
    let main = get("main");
    let bottom = get("bottomPanel");
    let secondary = get("secondarySidebar");
    let status = get("statusBar");
    let actions = get("topBarActions");
    let rail_node = rail.map(|value| registry.render_spec(value, entity, &view));
    let sidebar_node = sidebar.map(|value| registry.render_spec(value, entity, &view));
    let main_node = main.map(|value| registry.render_spec(value, entity, &view));
    let bottom_node = bottom.map(|value| registry.render_spec(value, entity, &view));
    let secondary_node = secondary.map(|value| registry.render_spec(value, entity, &view));
    let status_node = status.map(|value| registry.render_spec(value, entity, &view));
    let actions_node = actions.map(|value| registry.render_spec(value, entity, &view));
    let class = merge_class_name("almadar-dock-layout", &input.props);
    if compact {
        let open_drawer = *open.read();
        let show_left = open_drawer == Some(OpenDrawer::Left);
        let show_secondary = open_drawer == Some(OpenDrawer::Secondary);
        let show_bottom = open_drawer == Some(OpenDrawer::Bottom);
        let show_panel = *left_panel.read();
        let secondary_label = get_string(&input.props, "secondarySidebarLabel").unwrap_or_default();
        let bottom_label = get_string(&input.props, "bottomPanelLabel").unwrap_or_default();
        let open_menu_label = get_string(&input.props, "openMenuLabel").unwrap_or_default();
        let back_to_menu_label = get_string(&input.props, "backToMenuLabel").unwrap_or_default();
        let menu_label = get_string(&input.props, "menuLabel").unwrap_or_default();
        let close_label = get_string(&input.props, "closeDrawerLabel").unwrap_or_default();
        return rsx! {
            div { class: "{class}", style: "display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden",
                if rail.is_some() || sidebar.is_some() || secondary.is_some() || bottom.is_some() || actions.is_some() {
                    div { class: "almadar-dock-layout__top", style: "display:flex;align-items:center;gap:4px;flex-shrink:0",
                        if rail.is_some() || sidebar.is_some() {
                            button { class: "almadar-dock-layout__menu", aria_label: "{open_menu_label}", onclick: move |_| { left_panel.set(false); open.set(Some(OpenDrawer::Left)); }, "☰" }
                        }
                        div { style: "flex:1" }
                        if bottom.is_some() { button { class: "almadar-dock-layout__open-bottom", aria_label: "{bottom_label}", onclick: move |_| open.set(Some(OpenDrawer::Bottom)), "▤" } }
                        if secondary.is_some() { button { class: "almadar-dock-layout__open-secondary", aria_label: "{secondary_label}", onclick: move |_| open.set(Some(OpenDrawer::Secondary)), "◧" } }
                        {actions_node}
                    }
                }
                div { class: "almadar-dock-layout__main", style: "flex:1;min-height:0;min-width:0;overflow:auto", {main_node} }
                if status.is_some() { div { class: "almadar-dock-layout__status", {status_node} } }
                if open_drawer.is_some() { button { class: "almadar-dock-layout__backdrop", aria_label: "{close_label}", style: "position:absolute;inset:0;background:rgba(0,0,0,0.35)", onclick: move |_| open.set(None) } }
                if show_left {
                    div { class: "almadar-dock-layout__drawer almadar-dock-layout__drawer--left", style: "position:absolute;top:0;bottom:0;left:0;width:min(320px,85vw);overflow:auto;background:var(--almadar-bg)",
                        button { aria_label: "{close_label}", onclick: move |_| open.set(None), "×" }
                        h2 { "{menu_label}" }
                        if show_panel {
                            if rail.is_some() { button { aria_label: "{back_to_menu_label}", onclick: move |_| left_panel.set(false), "←" } }
                            {sidebar_node}
                        } else if rail.is_some() { {rail_node} } else { {sidebar_node} }
                    }
                }
                if show_secondary { div { class: "almadar-dock-layout__drawer almadar-dock-layout__drawer--right", style: "position:absolute;top:0;bottom:0;right:0;width:min(320px,85vw);overflow:auto;background:var(--almadar-bg)", button { aria_label: "{close_label}", onclick: move |_| open.set(None), "×" } {secondary_node} } }
                if show_bottom { div { class: "almadar-dock-layout__drawer almadar-dock-layout__drawer--bottom", style: "position:absolute;bottom:0;left:0;right:0;max-height:70vh;overflow:auto;background:var(--almadar-bg)", button { aria_label: "{close_label}", onclick: move |_| open.set(None), "×" } {bottom_node} } }
            }
        };
    }
    let rail_width = get_number_or(&input.props, "railWidth", 56.0);
    let sidebar_width = get_number_or(&input.props, "sidebarWidth", 20.0);
    let secondary_width = get_number_or(&input.props, "secondarySidebarWidth", 280.0);
    let bottom_height = get_number_or(&input.props, "bottomPanelHeight", 30.0);
    let show_sidebar = sidebar.is_some() && !get_bool_or(&input.props, "sidebarCollapsed", false);
    let show_secondary =
        secondary.is_some() && !get_bool_or(&input.props, "secondarySidebarCollapsed", false);
    let show_bottom = bottom.is_some() && !get_bool_or(&input.props, "bottomPanelCollapsed", false);
    rsx! {
        div { class: "{class}", style: "display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden",
            div { class: "almadar-dock-layout__body", style: "display:flex;flex:1;min-height:0",
                if rail.is_some() { div { class: "almadar-dock-layout__rail", style: "flex-shrink:0;width:{rail_width}px;overflow:auto", {rail_node} } }
                if show_sidebar { div { class: "almadar-dock-layout__sidebar", style: "flex-shrink:0;width:{sidebar_width}%;overflow:auto", {sidebar_node} } }
                div { class: "almadar-dock-layout__main", style: "flex:1;min-width:0;overflow:auto", {main_node} }
                if show_secondary { div { class: "almadar-dock-layout__secondary", style: "flex-shrink:0;width:{secondary_width}px;overflow:auto", {secondary_node} } }
            }
            if show_bottom { div { class: "almadar-dock-layout__bottom", style: "height:{bottom_height}%;overflow:auto", {bottom_node} } }
            if status.is_some() { div { class: "almadar-dock-layout__status", {status_node} } }
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
