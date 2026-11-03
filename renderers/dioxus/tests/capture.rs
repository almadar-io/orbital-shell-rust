//! The offscreen capture paints the mounted app to a PNG with no window.
#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use orbital_client_dioxus::offscreen::{capture_gpu_png, capture_png};
use orbital_client_dioxus::shell::mount_core;

fn program(main: serde_json::Value) -> String {
    serde_json::json!({
        "name": "t",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Row", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": [{ "name": "View", "linkedEntity": "Row", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", main]] }]
            } }],
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "View" }] }]
        }]
    })
    .to_string()
}

fn dark_pixels(path: &std::path::Path) -> usize {
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf[..info.buffer_size()]
        .chunks(4)
        .filter(|px| px[0] < 128 && px[1] < 128 && px[2] < 128)
        .count()
}

#[test]
fn activity_rail_folder_icon_uses_a_bundled_glyph_and_paints_pixels() {
    use anyrender::ImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;
    use fontique::{Blob, QueryStatus};
    use std::sync::Arc;

    fn icon_app() -> Element {
        let glyph = orbital_core::client::helpers::icon_name_to_glyph("folder");
        rsx! {
            div { style: "width:80px;height:64px;background:#ffffff",
                span { class: "almadar-activity-rail__icon", style: "font-size:32px;color:#000000", "{glyph}" }
            }
        }
    }

    fn blank_app() -> Element {
        rsx! { div { style: "width:80px;height:64px;background:#ffffff" } }
    }

    let glyph = orbital_core::client::helpers::icon_name_to_glyph("folder");
    let icon_char = glyph.chars().next().expect("folder glyph");
    assert_eq!(glyph.chars().count(), 1);
    let bundled_font = include_bytes!("../vendor/blitz-dom/assets/NotoSansSymbols2-Regular.ttf");
    let mut font_ctx = parley::FontContext::default();
    font_ctx
        .collection
        .register_fonts(Blob::new(Arc::new(bundled_font.as_slice()) as _), None);
    let mut query = font_ctx.collection.query(&mut font_ctx.source_cache);
    query.set_families(["Noto Sans Symbols 2"]);
    let mut covered = false;
    query.matches_with(|font| {
        covered = font
            .charmap()
            .and_then(|map| map.map(icon_char as u32))
            .is_some();
        QueryStatus::Stop
    });
    assert!(covered, "bundled symbol font lacks the shared folder glyph");

    fn painted_pixels(app: fn() -> Element) -> usize {
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig {
                viewport: Some(Viewport::new(80, 64, 1.0, ColorScheme::Light)),
                font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
                ..Default::default()
            },
        );
        doc.initial_build();
        doc.create_head_element(
            "style",
            &[],
            &Some(include_str!("../assets/css/almadar.css").into()),
        );
        doc.resolve(0.0);
        let mut renderer = anyrender_vello::VelloImageRenderer::new(80, 64);
        let mut pixels = vec![0u8; 80 * 64 * 4];
        renderer.render(
            |painter| paint_scene(painter, &doc, 1.0, 80, 64),
            &mut pixels,
        );
        pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0 && pixel[0] < 128 && pixel[1] < 128 && pixel[2] < 128)
            .count()
    }

    let icon_pixels = painted_pixels(icon_app);
    let blank_pixels = painted_pixels(blank_app);
    assert!(
        icon_pixels > blank_pixels + 10,
        "folder glyph painted {icon_pixels} dark pixels; blank painted {blank_pixels}"
    );
}

#[test]
fn text_on_the_main_slot_paints_dark_pixels_and_a_blank_page_does_not() {
    let dir = std::env::temp_dir().join(format!("dioxus-capture-{}", std::process::id()));
    mount_core(&program(
        serde_json::json!({ "type": "typography", "variant": "h1", "content": "Hello capture" }),
    ))
    .unwrap();
    let text = dir.join("text.png");
    capture_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "text painted {with_text} dark pixels");
    assert!(
        without < with_text / 10,
        "control: blank page painted {without}"
    );
}

#[test]
fn gpu_paints_the_same_slot_with_visible_text() {
    let dir = std::env::temp_dir().join(format!("dioxus-gpu-capture-{}", std::process::id()));
    mount_core(&program(
        serde_json::json!({ "type": "typography", "variant": "h1", "content": "GPU TODO" }),
    ))
    .unwrap();
    let text = dir.join("text.png");
    capture_gpu_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_gpu_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "GPU painted {with_text} dark pixels");
    assert!(
        without < with_text / 10,
        "control: blank page painted {without}"
    );
}

#[test]
fn pattern_tile_canvas_mapper_paints_headless_gpu_pixels() {
    use orbital_canvas::mappers::scene;
    use orbital_canvas::Canvas;

    let registry = orbital_client_dioxus::registry::default_registry();
    assert!(registry.supports("pattern-tile"));
    assert!(registry.is_partial("pattern-tile"));
    assert_eq!(
        registry.get_renderer("pattern-tile").pattern_name(),
        "pattern-tile"
    );
    let star = scene::map_scene(
        &serde_json::json!({
            "type": "pattern-tile", "variant": "star8", "size": 120,
            "color": "#101010", "strokeWidth": 4
        }),
        None,
    )
    .unwrap()
    .packet;
    let knot = scene::map_scene(
        &serde_json::json!({
            "type": "pattern-tile", "variant": "celtic-knot", "size": 120,
            "color": "#101010", "strokeWidth": 4
        }),
        None,
    )
    .unwrap()
    .packet;
    assert!(!star.draw_commands.is_empty());
    assert!(!knot.draw_commands.is_empty());
    let mut canvas = Canvas::new_headless().expect("PatternTile GPU adapter");
    let painted = canvas
        .render_packet_to_rgba(&star)
        .expect("star8 GPU paint");
    let other = canvas
        .render_packet_to_rgba(&knot)
        .expect("celtic-knot GPU paint");
    assert_eq!(painted.len(), (star.width * star.height * 4.0) as usize);
    assert!(
        painted.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "PatternTile GPU frame is blank"
    );
    assert_ne!(
        painted, other,
        "different shared variants painted identical GPU pixels"
    );
}

#[test]
fn positioned_drawer_paints_its_semantic_children_above_the_backdrop() {
    use anyrender::ImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { style: "position:relative;display:flex;flex-direction:column;width:100%;height:100%;min-height:100vh;overflow:hidden",
                div { "Main view" }
                button { style: "position:absolute;inset:0;background:rgba(0,0,0,0.35)" }
                div { class: "drawer", style: "position:absolute;top:0;bottom:0;left:0;width:min(320px,85vw);overflow:auto;background:white",
                    button { "×" }
                    h2 { "Menu" }
                    button { "Assets" }
                }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.resolve(0.0);
    let id = doc.query_selector(".drawer").unwrap().unwrap();
    let node = doc.get_node(id).unwrap();
    assert!(node.text_content().contains("Assets"));
    assert!(
        node.final_layout.size.width > 250.0,
        "drawer width: {}",
        node.final_layout.size.width
    );
    assert!(
        node.final_layout.size.height > 200.0,
        "drawer height: {}",
        node.final_layout.size.height
    );
    let mut renderer = anyrender_vello::VelloImageRenderer::new(400, 300);
    let mut pixels = vec![0u8; 400 * 300 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 300),
        &mut pixels,
    );
    let dark = pixels
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, rgba)| {
            index % 400 < 320 && rgba[0] < 128 && rgba[1] < 128 && rgba[2] < 128
        })
        .count();
    assert!(
        dark > 100,
        "drawer semantic text failed to paint: {dark} dark pixels"
    );
}

#[test]
fn live_document_replaces_text_after_signal_change() {
    use anyrender::ImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    thread_local! {
        static TEXT: std::cell::RefCell<Option<Signal<String>>> = const { std::cell::RefCell::new(None) };
    }

    fn app() -> Element {
        let text = use_signal(|| "No tasks yet".to_string());
        TEXT.with(|cell| *cell.borrow_mut() = Some(text));
        let current = text.read().clone();
        rsx! { div { "{current}" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 100, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet("div { display: block; color: black; font-size: 20px; }");
    doc.initial_build();
    doc.resolve(0.0);
    let html_id = doc.root_node().children[0];
    assert!(doc
        .get_node(html_id)
        .unwrap()
        .text_content()
        .contains("No tasks yet"));
    let mut renderer = anyrender_vello_cpu::VelloCpuImageRenderer::new(400, 100);
    let mut before = vec![0u8; 400 * 100 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 100),
        &mut before,
    );
    assert!(before.iter().any(|v| *v != 0), "initial text did not paint");

    TEXT.with(|cell| cell.borrow_mut().as_mut().unwrap().set("Tasks".to_string()));
    assert!(doc.poll(None));
    doc.resolve(0.0);
    let text = doc.get_node(html_id).unwrap().text_content();
    assert!(text.contains("Tasks"), "{text}");
    assert!(!text.contains("No tasks yet"), "{text}");
    let mut after = vec![0u8; 400 * 100 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 100),
        &mut after,
    );
    assert!(before != after, "text changed in DOM but not in paint");
}

#[test]
fn native_css_keyframes_change_computed_opacity_over_time() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated", "Animated" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(200, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; background: black; animation: entry-opacity 1s linear both; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let node_id = doc.query_selector(".animated").unwrap().unwrap();
    let opacity = |doc: &DioxusDocument| {
        doc.get_node(node_id)
            .unwrap()
            .primary_styles()
            .unwrap()
            .get_effects()
            .opacity
    };

    doc.resolve(0.0);
    let beginning = opacity(&doc);
    assert!(
        doc.is_animating(),
        "running CSS keyframes must request redraws"
    );
    doc.resolve(0.5);
    let middle = opacity(&doc);
    doc.resolve(1.0);
    let end = opacity(&doc);
    assert!(beginning < 0.1, "initial opacity: {beginning}");
    assert!(middle > 0.35 && middle < 0.65, "midpoint opacity: {middle}");
    assert!(end > 0.9, "final opacity: {end}");
}

#[test]
fn native_css_keyframes_change_gpu_pixels_over_time() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated", span { class: "tile" } } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; animation: entry-opacity 1s linear both; } \
         .tile { display: block; width: 100px; height: 80px; background: black; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let mut renderer = anyrender_vello::VelloImageRenderer::new(160, 100);
    let mut alpha_at = |time| {
        doc.resolve(time);
        let mut pixels = vec![0u8; 160 * 100 * 4];
        renderer.render(
            |painter| paint_scene(painter, &doc, 1.0, 160, 100),
            &mut pixels,
        );
        pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[3]))
            .sum::<u64>()
    };
    let beginning = alpha_at(0.0);
    let middle = alpha_at(0.5);
    let end = alpha_at(1.0);
    assert!(
        beginning < middle,
        "GPU opacity did not rise: {beginning}, {middle}, {end}"
    );
    assert!(
        middle < end,
        "GPU opacity did not finish: {beginning}, {middle}, {end}"
    );
}

#[test]
fn native_css_keyframes_fade_the_elements_own_background_on_gpu() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; background: black; animation: entry-opacity 1s linear both; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let mut renderer = anyrender_vello::VelloImageRenderer::new(160, 100);
    let mut alpha_at = |time| {
        doc.resolve(time);
        let mut pixels = vec![0u8; 160 * 100 * 4];
        renderer.render(
            |painter| paint_scene(painter, &doc, 1.0, 160, 100),
            &mut pixels,
        );
        pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[3]))
            .sum::<u64>()
    };
    let beginning = alpha_at(0.0);
    let middle = alpha_at(0.5);
    let end = alpha_at(1.0);
    assert!(
        beginning < middle,
        "GPU background opacity did not rise: {beginning}, {middle}, {end}"
    );
    assert!(
        middle < end,
        "GPU background opacity did not finish: {beginning}, {middle}, {end}"
    );
}

#[test]
fn native_entry_delay_uses_the_shared_theme_stylesheet() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "almadar-enter-fade almadar-enter-delay-3" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(&orbital_client_dioxus::styles::stylesheets()[0]);
    doc.initial_build();
    let node_id = doc.query_selector(".almadar-enter-fade").unwrap().unwrap();
    let opacity = |doc: &DioxusDocument| {
        doc.get_node(node_id)
            .unwrap()
            .primary_styles()
            .unwrap()
            .get_effects()
            .opacity
    };

    doc.resolve(0.0);
    let beginning = opacity(&doc);
    doc.resolve(0.1);
    let delayed = opacity(&doc);
    doc.resolve(0.3);
    let active = opacity(&doc);
    doc.resolve(0.5);
    let end = opacity(&doc);
    assert!(beginning < 0.1, "entry initial opacity: {beginning}");
    assert!(delayed < 0.1, "entry stagger delay did not hold: {delayed}");
    assert!(
        active > delayed,
        "entry did not progress: {delayed}, {active}"
    );
    assert!(end > 0.9, "entry did not finish: {end}");
}

#[test]
#[ignore = "tool: writes a PNG for a person to look at: ORB_CAPTURE_OUT=<file>"]
fn look() {
    let out = std::env::var("ORB_CAPTURE_OUT").expect("ORB_CAPTURE_OUT");
    mount_core(&program(serde_json::json!({ "type": "stack", "children": [
        { "type": "typography", "variant": "h1", "content": "Hello capture" },
        { "type": "button", "label": "Save", "event": "SAVE" }
    ] })))
    .unwrap();
    orbital_client_dioxus::app::CSS_TEXT
        .with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    capture_png(std::path::Path::new(&out), 600, 300).unwrap();
}
