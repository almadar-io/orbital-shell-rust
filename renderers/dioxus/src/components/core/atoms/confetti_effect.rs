use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::timer::animate;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::time::Duration;

pub struct ConfettiEffectRenderer;

#[derive(Props, Clone, PartialEq)]
struct ConfettiProps {
    value: Value,
}

#[allow(non_snake_case)]
fn Confetti(props: ConfettiProps) -> Element {
    let trigger = get_bool_or(&props.value, "trigger", false);
    let duration = get_number_or(&props.value, "duration", 2000.0).max(0.0) as u64;
    let count = get_number_or(&props.value, "particleCount", 30.0).max(0.0) as usize;
    let previous = use_hook(|| Rc::new(RefCell::new(false)));
    let mut progress = use_signal(|| None::<f64>);
    let mut generation = use_signal(|| 0_u64);
    let rising = trigger && !*previous.borrow();
    *previous.borrow_mut() = trigger;
    if rising {
        let next = generation.peek().saturating_add(1);
        generation.set(next);
        progress.set(Some(0.0));
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        spawn(async move {
            animate(Duration::from_millis(duration), move |fraction| {
                if *generation.peek() != next {
                    return false;
                }
                progress.set(Some(fraction));
                true
            })
            .await;
            if *generation.peek() == next {
                progress.set(None);
            }
        });
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        progress.set(None);
    }
    let Some(fraction) = progress() else {
        return rsx! {};
    };
    let colors = ["#FF5050", "#50C878", "#5078FF", "#FFC832", "#C850FF"];
    let class = merge_class_name("almadar-confetti-effect", &props.value);
    let opacity = (1.0 - fraction).max(0.0);
    rsx! {
        svg { class: "{class}", view_box: "0 0 160 80", width: "160", height: "80", "aria-hidden": "true",
            for i in 0..count {
                {
                    let angle = ((i * 137 + 31) % 360) as f64 * std::f64::consts::PI / 180.0;
                    let distance = 12.0 + ((i * 41) % 56) as f64;
                    let x = 80.0 + angle.cos() * distance * fraction;
                    let y = 40.0 + angle.sin() * distance * fraction + 18.0 * fraction * fraction;
                    let color = colors[i % colors.len()];
                    rsx! { rect { x: "{x}", y: "{y}", width: "4", height: "3", fill: "{color}", fill_opacity: "{opacity}" } }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for ConfettiEffectRenderer {
    fn pattern_name(&self) -> &str {
        "confetti-effect"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Confetti { value: props.clone() } }
    }
}
