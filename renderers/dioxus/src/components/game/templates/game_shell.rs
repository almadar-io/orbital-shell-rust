use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, merge_class_name};
use orbital_core::Value;

pub struct GameShellRenderer;

impl DioxusPatternRenderer for GameShellRenderer {
    fn pattern_name(&self) -> &str {
        "game-shell"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let fields = props.as_object();
        let child = |name: &str| fields.and_then(|p| p.get(name)).cloned();
        let children = child("children");
        let hud = child("hud");
        let addons = child("addons");
        let controls = child("controls");
        let overlay = child("overlay");
        let background = child("backgroundAsset");
        let hud_background = child("hudBackgroundAsset");
        let title = get_string(props, "appName");
        let show_top_bar = get_bool_or(props, "showTopBar", true);
        let class = merge_class_name("game-shell", props);
        let theme = get_string(props, "data-theme");
        let aria_label = get_string(props, "aria-label");
        let role = get_string(props, "role");
        let font = get_string(props, "fontFamily");
        let font_style = font
            .map(|value| format!("--font-family-display:{value};"))
            .unwrap_or_default();
        let style = format!("position:relative;width:100%;max-width:100vw;height:100vh;overflow:hidden;background:var(--color-background,#0a0a0f);color:var(--color-foreground,#e0e0e0);{font_style}");

        rsx! {
            div { class: "{class}", style: "{style}", "data-theme": theme, aria_label: aria_label, role: role,
                if let Some(asset) = background.as_ref() {
                    div { class: "game-shell__bg", style: "position:absolute;inset:0;z-index:0;opacity:0.18;pointer-events:none;",
                        {crate::components::game::asset_panel::fill_parent(asset, "repeat", view, registry)}
                    }
                }
                div { class: "game-shell__content", style: "position:absolute;inset:0;z-index:1;",
                    if let Some(spec) = children.as_ref() { {registry.render_spec(spec, entity, view)} }
                }
                if show_top_bar || hud.is_some() {
                    div { class: "game-shell__top", style: "position:absolute;top:12px;left:12px;right:12px;z-index:2;display:flex;align-items:flex-start;gap:12px;pointer-events:none;",
                        if show_top_bar {
                            div { class: "game-shell__title", style: "position:relative;padding:6px 16px;flex-shrink:0;pointer-events:auto;",
                                if let Some(asset) = hud_background.as_ref() {
                                    div { style: "position:absolute;inset:0;z-index:0;pointer-events:none;",
                                        {crate::components::game::asset_panel::fill_parent(asset, "stretch", view, registry)}
                                    }
                                }
                                if let Some(name) = title.as_ref() {
                                    span { class: "font-display", style: "position:relative;z-index:1;font-weight:700;font-size:1.05rem;letter-spacing:0.06em;white-space:nowrap;", "{name}" }
                                }
                            }
                        }
                        if let Some(spec) = hud.as_ref() {
                            div { class: "game-shell__hud", style: "flex:1;min-width:0;pointer-events:auto;", {registry.render_spec(spec, entity, view)} }
                        }
                    }
                }
                if let Some(spec) = controls.as_ref() {
                    div { class: "game-shell__controls", style: "position:absolute;left:16px;bottom:16px;z-index:2;display:flex;flex-direction:column;align-items:flex-start;gap:10px;pointer-events:auto;",
                        {registry.render_spec(spec, entity, view)}
                    }
                }
                if let Some(spec) = addons.as_ref() {
                    div { class: "game-shell__actions", style: "position:absolute;right:16px;bottom:16px;z-index:2;display:flex;flex-direction:column;align-items:flex-end;gap:10px;pointer-events:auto;",
                        {registry.render_spec(spec, entity, view)}
                    }
                }
                if let Some(spec) = overlay.as_ref() {
                    div { class: "game-shell__overlay", style: "position:absolute;inset:0;z-index:3;display:flex;align-items:center;justify-content:center;pointer-events:none;",
                        div { style: "pointer-events:auto;", {registry.render_spec(spec, entity, view)} }
                    }
                }
            }
        }
    }
}
