//! Native Dioxus body for the `vote-stack` pattern, mirroring `VoteStack.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct VoteStackRenderer;

impl VoteStackRenderer {
    const NAME: &str = "vote-stack";
}

impl DioxusPatternRenderer for VoteStackRenderer {
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
        let size = get_string_or(props, "size", "md");
        let (button, text, icon_size) = match size.as_str() {
            "sm" => ("w-7 h-7", "text-sm min-w-[2rem]", "xs"),
            "lg" => ("w-11 h-11", "text-lg min-w-[3rem]", "md"),
            _ => ("w-9 h-9", "text-base min-w-[2.5rem]", "sm"),
        };
        let vertical = get_string_or(props, "variant", "vertical") != "horizontal";
        let disabled = get_bool_or(props, "disabled", false);
        let user_vote = get_string(props, "userVote");
        let is_up = user_vote.as_deref() == Some("up");
        let is_down = user_vote.as_deref() == Some("down");
        let count = props.get("count").map(format_value).unwrap_or_default();
        let direction = if vertical { "flex-col" } else { "flex-row" };
        let class = merge_class_name(
            &format!(
                "inline-flex items-center justify-center w-fit {direction} rounded-interactive border-[length:var(--border-width)] border-border bg-surface {}",
                if disabled { "opacity-50 cursor-not-allowed" } else { "" }
            ),
            props,
        );
        let label = get_string(props, "aria-label")
            .or_else(|| get_string(props, "label"))
            .unwrap_or_else(|| crate::i18n::t("voteStack.ariaLabel"));
        let button_base = "inline-flex items-center justify-center hover:bg-muted active:bg-muted transition-colors duration-instant disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset";
        let up_class = format!(
            "{button_base} {} {} {button}",
            if vertical {
                "rounded-t-interactive"
            } else {
                "rounded-l-interactive"
            },
            if is_up {
                "text-primary"
            } else {
                "text-muted-foreground"
            }
        );
        let down_class = format!(
            "{button_base} {} {} {button}",
            if vertical {
                "rounded-b-interactive"
            } else {
                "rounded-r-interactive"
            },
            if is_down {
                "text-primary"
            } else {
                "text-muted-foreground"
            }
        );
        let count_class = format!(
            "text-center font-bold tabular-nums select-none text-foreground {} px-1 py-0.5 {text}",
            if vertical {
                "border-y-[length:var(--border-width)] border-border w-full"
            } else {
                "border-x-[length:var(--border-width)] border-border h-full"
            }
        );
        let up_icon = pattern_spec(
            "icon",
            [
                ("name", Value::string("chevron-up")),
                ("size", Value::string(icon_size)),
            ],
        );
        let down_icon = pattern_spec(
            "icon",
            [
                ("name", Value::string("chevron-down")),
                ("size", Value::string(icon_size)),
            ],
        );
        let event = get_string(props, "voteEvent");
        let sink = view.sink;
        let emit = move |next: Option<&str>| {
            if let (Some(sink), Some(event)) = (sink, &event) {
                let next = next.map(Value::string).unwrap_or(Value::Null);
                sink.call((event.clone(), object_value([("next", next)])));
            }
        };
        let emit_down = emit.clone();
        let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
            .into_iter()
            .filter(|attr| !matches!(attr.name, "role" | "aria-label"))
            .collect();
        rsx! {
            div { class: "{class}", role: "group", "aria-label": "{label}", ..attrs,
                button {
                    r#type: "button",
                    class: "{up_class}",
                    disabled: disabled,
                    "aria-label": crate::i18n::t("aria.upvote"),
                    "aria-pressed": "{is_up}",
                    onclick: move |_| emit(if is_up { None } else { Some("up") }),
                    {registry.render_spec(&up_icon, None, view)}
                }
                span { class: "{count_class}", "aria-live": "polite", "aria-atomic": "true", "{count}" }
                button {
                    r#type: "button",
                    class: "{down_class}",
                    disabled: disabled,
                    "aria-label": crate::i18n::t("aria.downvote"),
                    "aria-pressed": "{is_down}",
                    onclick: move |_| emit_down(if is_down { None } else { Some("down") }),
                    {registry.render_spec(&down_icon, None, view)}
                }
            }
        }
    }
}
