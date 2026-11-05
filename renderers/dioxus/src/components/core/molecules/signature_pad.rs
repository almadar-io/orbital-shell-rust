use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `signature-pad` pattern.
pub struct SignaturePadRenderer;

#[derive(Props, Clone, PartialEq)]
struct SignaturePadProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const CLEAR: &str = "__signature_pad_clear";

#[allow(non_snake_case)]
fn SignaturePad(input: SignaturePadProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let class_name = get_string(props, "className").unwrap_or_default();
    let initial = get_string(props, "value").is_some_and(|value| !value.is_empty());
    let mut has_signature = use_signal(move || initial);
    let clear_event = get_string(props, "clearEvent").filter(|name| !name.is_empty());
    let sign_event = get_string(props, "signEvent").filter(|name| !name.is_empty());
    let sink = view.sink;
    let relay: EventSink = Callback::new(move |(name, _): (String, Value)| {
        if name == CLEAR {
            has_signature.set(false);
            if let (Some(sink), Some(event)) = (sink, &clear_event) {
                sink.call((event.clone(), object_value::<&str>([])));
            }
        }
    });

    if get_bool_or(props, "isLoading", false) {
        let spec = pattern_spec(
            "loading-state",
            [
                ("message", Value::string("Loading...")),
                ("className", Value::string(class_name)),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }
    if let Some(error) = props.get("error").filter(|error| !error.is_null()) {
        let message = error
            .as_str()
            .map(str::to_string)
            .or_else(|| get_string(error, "message"))
            .unwrap_or_default();
        let spec = pattern_spec(
            "error-state",
            [
                ("title", Value::string("Signature pad error")),
                ("message", Value::string(message)),
                ("className", Value::string(class_name)),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }

    let label = get_string(props, "label").unwrap_or_else(|| "Signature".to_string());
    let helper =
        get_string(props, "helperText").unwrap_or_else(|| "Draw your signature above".to_string());
    let read_only = get_bool_or(props, "readOnly", false);
    let height = get_number_or(props, "height", 200.0) as i64;
    let card_class = cn(&[
        "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md p-4",
        &class_name,
    ]);
    let pad_class = cn(&[
        "bg-background rounded-container shadow-none w-full border border-border",
        if read_only {
            "opacity-60 cursor-not-allowed"
        } else {
            "cursor-crosshair"
        },
    ]);
    let aria_label = if label.is_empty() {
        "Signature".to_string()
    } else {
        label.clone()
    };
    let label_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("label")),
            ("weight", Value::string("medium")),
            ("content", Value::string(label.clone())),
        ],
    );
    let helper_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("caption")),
            ("color", Value::string("secondary")),
            ("content", Value::string(helper.clone())),
        ],
    );
    let mut buttons = vec![pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("eraser")),
            ("label", Value::string("Clear")),
            ("action", Value::string(CLEAR)),
            ("disabled", Value::Bool(!has_signature())),
        ],
    )];
    if sign_event.is_some() {
        buttons.push(pattern_spec(
            "button",
            [
                ("variant", Value::string("primary")),
                ("size", Value::string("sm")),
                ("leftIcon", Value::string("check")),
                ("label", Value::string("Confirm")),
                ("disabled", Value::Bool(!has_signature())),
            ],
        ));
    }
    let actions = pattern_spec(
        "hstack",
        [
            ("gap", Value::string("sm")),
            ("justify", Value::string("end")),
            ("children", array_value(buttons)),
        ],
    );
    let relay_view = view.for_source(view.source_trait.clone(), relay);
    rsx! {
        div { class: "{card_class}", style: "height:auto",
            div { class: "flex flex-col gap-2 items-stretch justify-start", style: "height:auto",
                if !label.is_empty() { {registry.render_spec(&label_spec, None, &view)} }
                div { class: "{pad_class}",
                    canvas { class: "w-full touch-none", width: "600", height: "{height}", style: "height:{height}px", role: "img", aria_label: "{aria_label}" }
                }
                if !helper.is_empty() { {registry.render_spec(&helper_spec, None, &view)} }
                if !read_only { {registry.render_spec(&actions, None, &relay_view)} }
            }
        }
    }
}

impl DioxusPatternRenderer for SignaturePadRenderer {
    fn pattern_name(&self) -> &str {
        "signature-pad"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { SignaturePad { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
