use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, pattern_spec, render_with_local_sink, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const API_UNAVAILABLE: &str = "Camera API not available in this environment";
const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct QrScannerRenderer;

impl QrScannerRenderer {
    const NAME: &str = "qr-scanner";
}

#[derive(Props, Clone, PartialEq)]
struct QrProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn control(action: &str, label: &str, icon: &str) -> Value {
    pattern_spec(
        "button",
        [
            ("action", Value::string(action)),
            ("aria-label", Value::string(label)),
            (
                "className",
                Value::string("rounded-full bg-black bg-opacity-60 p-2 text-white hover:bg-opacity-80 focus:outline-none focus-visible:ring-2 focus-visible:ring-white"),
            ),
            (
                "children",
                Value::Array(
                    vec![pattern_spec(
                        "icon",
                        [("name", Value::string(icon)), ("className", Value::string("h-4 w-4"))],
                    )]
                    .into(),
                ),
            ),
        ],
    )
}

fn text(variant: &str, class: &str, content: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("variant", Value::string(variant)),
            ("className", Value::string(class)),
            ("content", Value::string(content)),
        ],
    )
}

#[allow(non_snake_case)]
fn QrScanner(input: QrProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let initial_paused = get_bool_or(props, "paused", false);
    let mut paused = use_signal(|| initial_paused);
    let mut facing = use_signal(|| get_string_or(props, "facingMode", "environment"));
    let mut error = use_signal(|| (!initial_paused).then(|| API_UNAVAILABLE.to_string()));
    let show_overlay = get_bool_or(props, "showOverlay", true);
    let show_controls = get_bool_or(props, "showCameraControls", true);
    let class_name = get_string_or(props, "className", "");
    let is_paused = *paused.read();
    let is_ready = false;
    let current_error = error.read().clone();
    let fallback = props
        .get("fallback")
        .filter(|node| !matches!(node, Value::Null));

    if let (Some(_), Some(fallback)) = (current_error.as_ref(), fallback) {
        let class = cn(&[BOX, "relative", &class_name]);
        return rsx! { div { class: "{class}", {view.render_node(registry, fallback)} } };
    }

    let root = cn(&[
        BOX,
        "relative overflow-hidden rounded-sm",
        "bg-black",
        "aspect-square w-full max-w-md",
        &class_name,
    ]);
    let video = cn(&[BOX, "absolute w-full h-full", "inset-0 object-cover"]);
    let aria = get_string_or(props, "aria-label", &crate::i18n::t("aria.qrScanner"));
    let pause_label = if is_paused {
        crate::i18n::t("qrScanner.resumeScanning")
    } else {
        crate::i18n::t("qrScanner.pauseScanning")
    };
    let pause_icon = if is_paused { "play" } else { "pause" };
    let facing_label = if *facing.read() == "environment" {
        crate::i18n::t("qrScanner.switchToFrontCamera")
    } else {
        crate::i18n::t("qrScanner.switchToRearCamera")
    };
    let controls_el = show_controls.then(|| {
        let spec = pattern_spec(
            "hstack",
            [(
                "children",
                Value::Array(
                    vec![
                        control("__qr_pause", &pause_label, pause_icon),
                        control("__qr_facing", &facing_label, "refresh-cw"),
                    ]
                    .into(),
                ),
            )],
        );
        render_with_local_sink(&view, registry, &spec, move |event, _| {
            let mut paused = paused;
            let mut error = error;
            let mut facing = facing;
            match event {
                "__qr_pause" => {
                    let next = !*paused.peek();
                    paused.set(next);
                    if next {
                        error.set(None);
                    } else {
                        error.set(Some(API_UNAVAILABLE.to_string()));
                    }
                    true
                }
                "__qr_facing" => {
                    let next = if *facing.peek() == "environment" {
                        "user"
                    } else {
                        "environment"
                    };
                    facing.set(next.to_string());
                    true
                }
                _ => false,
            }
        })
    });
    let err_wrap = cn(&[
        BOX,
        "absolute flex p-6",
        "inset-0 flex-col items-center justify-center gap-2 bg-black bg-opacity-80 text-center",
    ]);
    let pause_wrap = cn(&[
        BOX,
        "absolute flex",
        "inset-0 items-center justify-center bg-black bg-opacity-60",
    ]);
    let overlay_wrap = cn(&[BOX, "absolute", "pointer-events-none inset-0"]);
    let ctl_wrap = cn(&[
        BOX,
        "absolute flex",
        "bottom-3 left-1/2 -translate-x-1/2 gap-2",
    ]);
    let corners = [
        "left-[15%] top-[15%] h-8 w-8 border-l-heavy border-t-heavy border-white",
        "right-[15%] top-[15%] h-8 w-8 border-r-heavy border-t-heavy border-white",
        "bottom-[15%] left-[15%] h-8 w-8 border-b-heavy border-l-heavy border-white",
        "bottom-[15%] right-[15%] h-8 w-8 border-b-heavy border-r-heavy border-white",
        "left-[15%] right-[15%] top-1/2 h-px bg-white opacity-60",
    ]
    .map(|c| cn(&[BOX, "absolute", c]));
    let camera_icon = pattern_spec(
        "icon",
        [
            ("name", Value::string("camera")),
            ("className", Value::string("h-8 w-8 text-white")),
            ("aria-hidden", Value::Bool(true)),
        ],
    );
    rsx! {
        div { class: "{root}", "data-pattern": "qr-scanner", role: "region", "aria-label": "{aria}",
            video { class: "{video}", "aria-hidden": "true", playsinline: true, muted: true }
            if show_overlay && is_ready && !is_paused {
                div { class: "{overlay_wrap}",
                    for c in corners.iter() { div { class: "{c}" } }
                }
            }
            if let Some(message) = current_error.as_ref() {
                div { class: "{err_wrap}",
                    {registry.render_spec(&camera_icon, None, &view)}
                    {registry.render_spec(&text("body2", "text-white", &crate::i18n::t("qrScanner.cameraUnavailable")), None, &view)}
                    {registry.render_spec(&text("caption", "text-white opacity-70", message), None, &view)}
                }
            }
            if is_paused && current_error.is_none() {
                div { class: "{pause_wrap}",
                    {registry.render_spec(&text("body2", "text-white", &crate::i18n::t("qrScanner.paused")), None, &view)}
                }
            }
            if let Some(controls_el) = controls_el {
                div { class: "{ctl_wrap}", {controls_el} }
            }
        }
    }
}

impl DioxusPatternRenderer for QrScannerRenderer {
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
        rsx! { QrScanner { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
