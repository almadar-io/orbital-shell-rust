//! Native Dioxus body for the `number-stepper` pattern, mirroring `NumberStepper.tsx`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{
    number_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use crate::timer::Delay;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};
use std::time::Duration;

pub struct NumberStepperRenderer;

impl NumberStepperRenderer {
    const NAME: &str = "number-stepper";
}

/// How long a press waits before it repeats, then how often it repeats, as the web's `LONG_PRESS_*`.
const LONG_PRESS_DELAY_MS: u64 = 400;
const LONG_PRESS_INTERVAL_MS: u64 = 100;

/// The value one more step of a held button reaches from `current`, snapped and clamped as `emit` does; `None` once a
/// bound stops it.
pub(crate) fn held_step(
    current: f64,
    delta: f64,
    step: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> Option<f64> {
    let stepped = (((current + delta) / step) + 0.5).floor() * step;
    let clamped = stepped
        .min(max.unwrap_or(f64::INFINITY))
        .max(min.unwrap_or(f64::NEG_INFINITY));
    (clamped != current).then_some(clamped)
}

#[derive(Props, Clone, PartialEq)]
struct StepperProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn js_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        number.to_string()
    }
}

impl DioxusPatternRenderer for NumberStepperRenderer {
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
        rsx! { Stepper { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[allow(non_snake_case)]
fn Stepper(input: StepperProps) -> Element {
    let view = &input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut holding = use_signal(|| 0u64);
    let size = get_string_or(props, "size", "md");
    let (button, text, icon_size) = match size.as_str() {
        "sm" => ("w-7 h-7", "text-sm min-w-[2rem]", "xs"),
        "lg" => ("w-11 h-11", "text-lg min-w-[3rem]", "md"),
        _ => ("w-9 h-9", "text-base min-w-[2.5rem]", "sm"),
    };
    let value = get_number_or(props, "value", 0.0);
    let min = get_number(props, "min");
    let max = get_number(props, "max");
    let step = get_number_or(props, "step", 1.0);
    let disabled = get_bool_or(props, "disabled", false);
    let at_min = min.is_some_and(|min| value <= min);
    let at_max = max.is_some_and(|max| value >= max);
    let class = merge_class_name(
            &format!(
                "inline-flex items-center rounded-interactive border-[length:var(--border-width)] border-border bg-surface {}",
                if disabled { "opacity-50 cursor-not-allowed" } else { "" }
            ),
            props,
        );
    let label = get_string(props, "aria-label")
        .or_else(|| get_string(props, "label"))
        .unwrap_or_else(|| "Number stepper".to_string());
    let button_base = "inline-flex items-center justify-center text-foreground hover:bg-muted active:bg-muted transition-colors duration-instant disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset";
    let minus_class = format!("{button_base} rounded-l-interactive {button}");
    let plus_class = format!("{button_base} rounded-r-interactive {button}");
    let value_class = format!("text-center font-medium tabular-nums text-foreground border-x-[length:var(--border-width)] border-border px-1 select-none {text}");
    let minus_icon = pattern_spec(
        "icon",
        [
            ("name", Value::string("minus")),
            ("size", Value::string(icon_size)),
        ],
    );
    let plus_icon = pattern_spec(
        "icon",
        [
            ("name", Value::string("plus")),
            ("size", Value::string(icon_size)),
        ],
    );
    let action = get_string(props, "action");
    let payload = props
        .as_object()
        .and_then(|fields| fields.get("actionPayload"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(ValueMap::new);
    let sink = view.sink;
    let emit = move |target: f64| {
        let (Some(sink), Some(action)) = (sink, &action) else {
            return;
        };
        let stepped = ((target / step) + 0.5).floor() * step;
        let clamped = stepped
            .min(max.unwrap_or(f64::INFINITY))
            .max(min.unwrap_or(f64::NEG_INFINITY));
        let mut fields = payload.clone();
        fields.insert("value".into(), number_value(clamped));
        sink.call((action.clone(), Value::Object(fields.into())));
    };
    let emit_plus = emit.clone();
    // Holding a button repeats the step: after the delay, every interval, from the value it last reached (the web's
    // interval re-emits the value it captured at press time, so it never passes one step).
    let mut hold = {
        let emit = emit.clone();
        move |delta: f64| {
            let mine = holding.peek().wrapping_add(1);
            holding.set(mine);
            let emit = emit.clone();
            spawn(async move {
                Delay::new(Duration::from_millis(LONG_PRESS_DELAY_MS)).await;
                let mut current = value;
                while *holding.peek() == mine {
                    let Some(next) = held_step(current, delta, step, min, max) else {
                        break;
                    };
                    emit(next);
                    current = next;
                    Delay::new(Duration::from_millis(LONG_PRESS_INTERVAL_MS)).await;
                }
            });
        }
    };
    let mut hold_plus = hold.clone();
    let release = move |_| {
        let next = holding.peek().wrapping_add(1);
        holding.set(next);
    };
    let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
        .into_iter()
        .filter(|attr| !matches!(attr.name, "role" | "aria-label"))
        .collect();
    let shown = js_number(value);
    rsx! {
        div { class: "{class}", role: "group", "aria-label": "{label}", ..attrs,
            button {
                r#type: "button",
                class: "{minus_class}",
                disabled: disabled || at_min,
                "aria-label": "Decrease",
                onclick: move |_| { if !at_min { emit(value - step); } },
                onmousedown: move |_| { if !at_min && !disabled { hold(-step); } },
                onmouseup: release,
                onmouseleave: release,
                {registry.render_spec(&minus_icon, None, view)}
            }
            span { class: "{value_class}", "aria-live": "polite", "aria-atomic": "true", "{shown}" }
            button {
                r#type: "button",
                class: "{plus_class}",
                disabled: disabled || at_max,
                "aria-label": "Increase",
                onclick: move |_| { if !at_max { emit_plus(value + step); } },
                onmousedown: move |_| { if !at_max && !disabled { hold_plus(step); } },
                onmouseup: release,
                onmouseleave: release,
                {registry.render_spec(&plus_icon, None, view)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_step_moves_from_where_it_is_and_stops_at_a_bound() {
        assert_eq!(held_step(3.0, 1.0, 1.0, None, None), Some(4.0));
        assert_eq!(held_step(4.0, 1.0, 1.0, None, Some(5.0)), Some(5.0));
        assert_eq!(held_step(5.0, 1.0, 1.0, None, Some(5.0)), None, "control: at the bound it stops");
        assert_eq!(held_step(0.2, -0.5, 0.5, Some(0.0), None), Some(0.0), "snapped then clamped");
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod dom_tests {
    use super::*;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_native::DioxusDocument;
    use keyboard_types::Modifiers;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static EVENTS: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
    }

    fn app() -> Element {
        let props: Value = serde_json::from_value(serde_json::json!({ "type": "number-stepper", "value": 3, "max": 10, "action": "STEPPED" })).unwrap();
        let entities = HashMap::new();
        let trait_content = ValueMap::new();
        let sink = Callback::new(|event| EVENTS.with(|events| events.borrow_mut().push(event)));
        let view = DioxusView {
            trait_name: "Stepper",
            orbital_name: "Stepper",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        crate::registry::default_registry().render_spec(&props, None, &view)
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..4 {
            if !doc.poll(None) {
                break;
            }
        }
        doc.resolve(0.0);
    }

    fn mouse(x: f32, y: f32, buttons: MouseEventButtons) -> BlitzMouseButtonEvent {
        BlitzMouseButtonEvent { x, y, button: MouseEventButton::Main, buttons, mods: Modifiers::default() }
    }

    fn reactor() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().unwrap()
    }

    fn document() -> DioxusDocument {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig { viewport: Some(Viewport::new(400, 200, 1.0, ColorScheme::Light)), ..Default::default() },
        );
        for css in crate::styles::stylesheets() {
            doc.add_user_agent_stylesheet(&css);
        }
        doc.initial_build();
        settle(&mut doc);
        doc
    }

    fn stepped() -> Vec<f64> {
        EVENTS.with(|events| {
            events.borrow().iter().filter(|(name, _)| name == "STEPPED").filter_map(|(_, p)| p.get("value").and_then(Value::as_number)).collect()
        })
    }

    /// Press Increase, keep it held for `held`, then release.
    fn press_increase(doc: &mut DioxusDocument, held: std::time::Duration) {
        let id = doc.query_selector("[aria-label=\"Increase\"]").unwrap().expect("increase");
        let at = doc.get_node(id).unwrap().synthetic_click_event_data(Modifiers::default());
        doc.handle_ui_event(UiEvent::MouseMove(mouse(at.x, at.y, MouseEventButtons::None)));
        doc.handle_ui_event(UiEvent::MouseDown(mouse(at.x, at.y, MouseEventButtons::Primary)));
        let until = std::time::Instant::now() + held;
        while std::time::Instant::now() < until {
            settle(doc);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        doc.handle_ui_event(UiEvent::MouseUp(mouse(at.x, at.y, MouseEventButtons::None)));
        settle(doc);
        std::thread::sleep(std::time::Duration::from_millis(250));
        settle(doc);
    }

    #[test]
    fn holding_increase_repeats_from_the_value_it_reached() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let reactor = reactor();
        let _inside = reactor.enter();
        let mut doc = document();
        press_increase(&mut doc, std::time::Duration::from_millis(750));
        let values = stepped();
        assert!(values.len() >= 3, "a 750 ms hold repeats after 400 ms: {values:?}");
        assert_eq!(&values[..3], &[4.0, 5.0, 6.0], "each repeat moves on from the last: {values:?}");
    }

    #[test]
    fn control_a_quick_press_steps_once() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let reactor = reactor();
        let _inside = reactor.enter();
        let mut doc = document();
        press_increase(&mut doc, std::time::Duration::from_millis(0));
        assert_eq!(stepped(), vec![4.0], "the click's one step, no repeat after release");
    }
}
