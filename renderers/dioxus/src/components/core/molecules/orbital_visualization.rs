use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct OrbitalVisualizationRenderer;

impl OrbitalVisualizationRenderer {
    const NAME: &str = "orbital-visualization";
}

struct OrbitalConfig {
    name: &'static str,
    series: u8,
    lobes: u8,
    has_node: bool,
    scale: f64,
}

fn config_for(complexity: f64) -> OrbitalConfig {
    let (name, series, lobes, has_node, scale) = if complexity <= 3.0 {
        ("1s Orbital", 1, 1, false, 0.6)
    } else if complexity <= 8.0 {
        ("2s Orbital", 2, 1, false, 0.8)
    } else if complexity <= 15.0 {
        ("2p Orbital", 3, 2, false, 1.0)
    } else if complexity <= 25.0 {
        ("3s Orbital", 4, 1, true, 1.0)
    } else if complexity <= 40.0 {
        ("3p Orbital", 5, 2, true, 1.1)
    } else if complexity <= 60.0 {
        ("3d Orbital", 6, 4, true, 1.2)
    } else {
        ("4f Orbital", 7, 6, true, 1.3)
    };
    OrbitalConfig {
        name,
        series,
        lobes,
        has_node,
        scale,
    }
}

fn calculate_complexity(schema: Option<&Value>) -> f64 {
    let Some(schema) = schema.filter(|schema| schema.as_object().is_some()) else {
        return 1.0;
    };
    let entities = get_number_or(schema, "dataEntities", 0.0);
    let traits = get_number_or(schema, "traits", 0.0);
    let pages = schema
        .get("ui")
        .map(|ui| get_array(ui, "pages"))
        .unwrap_or_default();
    let sections: f64 = pages
        .iter()
        .map(|page| get_number_or(page, "sections", 0.0))
        .sum();
    entities * 3.0 + pages.len() as f64 * 2.0 + traits * 2.0 + sections
}

fn color(series: u8) -> String {
    format!("var(--color-series-{series})")
}

fn glow(series: u8) -> String {
    format!("color-mix(in srgb, var(--color-series-{series}) 30%, transparent)")
}

fn sphere(config: &OrbitalConfig, size: f64, animated: bool) -> Element {
    let sphere_size = size * config.scale * 0.4;
    let c = color(config.series);
    let g = glow(config.series);
    let class = cn(&[BOX, "absolute rounded-full"]);
    let animation = if animated {
        "animation:orbital-pulse 3s ease-in-out infinite;"
    } else {
        ""
    };
    let style = format!(
        "width:{sphere_size}px;height:{sphere_size}px;background:radial-gradient(circle at 30% 30%, color-mix(in srgb, {c} 87%, transparent), color-mix(in srgb, {c} 53%, transparent) 50%, color-mix(in srgb, {c} 27%, transparent) 100%);box-shadow:inset -10px -10px 20px color-mix(in srgb, var(--color-foreground) 30%, transparent), inset 5px 5px 15px color-mix(in srgb, var(--surface-diagram) 20%, transparent), 0 0 {}px {g}, 0 0 {}px {g};left:50%;top:50%;transform:translate(-50%, -50%);{animation}",
        size * 0.15,
        size * 0.3
    );
    rsx! { div { class: "{class}", style: "{style}" } }
}

fn dumbbell(config: &OrbitalConfig, size: f64, animated: bool, rotation: f64) -> Element {
    let lobe_size = size * config.scale * 0.25;
    let offset = size * 0.18;
    let c = color(config.series);
    let g = glow(config.series);
    let outer_class = cn(&[BOX, "absolute"]);
    let lobe_class = cn(&[BOX, "absolute rounded-full"]);
    let node_class = cn(&[BOX, "absolute rounded-full bg-foreground"]);
    let animation = if animated {
        "animation:orbital-rotate 8s linear infinite;"
    } else {
        ""
    };
    let outer = format!("width:{size}px;height:{size}px;left:50%;top:50%;transform:translate(-50%, -50%) rotate({rotation}deg);{animation}");
    let top = format!(
        "width:{lobe_size}px;height:{}px;background:radial-gradient(ellipse at 50% 30%, color-mix(in srgb, {c} 87%, transparent), color-mix(in srgb, {c} 40%, transparent) 70%, transparent 100%);box-shadow:0 0 {}px {g};left:50%;top:calc(50% - {offset}px);transform:translate(-50%, -100%);border-radius:50% 50% 40% 40%;",
        lobe_size * 1.4,
        size * 0.1
    );
    let bottom = format!(
        "width:{lobe_size}px;height:{}px;background:radial-gradient(ellipse at 50% 70%, color-mix(in srgb, {c} 87%, transparent), color-mix(in srgb, {c} 40%, transparent) 70%, transparent 100%);box-shadow:0 0 {}px {g};left:50%;bottom:calc(50% - {offset}px);transform:translate(-50%, 100%);border-radius:40% 40% 50% 50%;",
        lobe_size * 1.4,
        size * 0.1
    );
    let node = format!(
        "width:{}px;height:{}px;left:50%;top:50%;transform:translate(-50%, -50%);box-shadow:0 0 {}px color-mix(in srgb, var(--surface-diagram) 80%, transparent);",
        size * 0.06,
        size * 0.06,
        size * 0.05
    );
    let has_node = config.has_node;
    rsx! {
        div { class: "{outer_class}", style: "{outer}",
            div { class: "{lobe_class}", style: "{top}" }
            div { class: "{lobe_class}", style: "{bottom}" }
            if has_node { div { class: "{node_class}", style: "{node}" } }
        }
    }
}

fn cloverleaf(config: &OrbitalConfig, size: f64, animated: bool) -> Element {
    let lobes = config.lobes as usize;
    let angle_step = 360.0 / lobes as f64;
    let lobe_size = size * 0.18;
    let lobe_distance = size * 0.22;
    let c = color(config.series);
    let g = glow(config.series);
    let outer_class = cn(&[BOX, "absolute"]);
    let lobe_class = cn(&[BOX, "absolute rounded-full"]);
    let node_class = cn(&[BOX, "absolute rounded-full bg-foreground"]);
    let animation = if animated {
        "animation:orbital-rotate 12s linear infinite;"
    } else {
        ""
    };
    let outer = format!("width:{size}px;height:{size}px;left:50%;top:50%;transform:translate(-50%, -50%);{animation}");
    let lobe_styles: Vec<String> = (0..lobes)
        .map(|i| {
            let angle = i as f64 * angle_step * (std::f64::consts::PI / 180.0);
            let x = angle.cos() * lobe_distance;
            let y = angle.sin() * lobe_distance;
            format!(
                "width:{lobe_size}px;height:{}px;background:radial-gradient(ellipse at 50% 40%, color-mix(in srgb, {c} 87%, transparent), color-mix(in srgb, {c} 33%, transparent) 80%, transparent 100%);box-shadow:0 0 {}px {g};left:calc(50% + {x}px);top:calc(50% + {y}px);transform:translate(-50%, -50%) rotate({}deg);",
                lobe_size * 1.3,
                size * 0.08,
                i as f64 * angle_step + 90.0
            )
        })
        .collect();
    let node = format!(
        "width:{}px;height:{}px;left:50%;top:50%;transform:translate(-50%, -50%);box-shadow:0 0 {}px color-mix(in srgb, var(--surface-diagram) 90%, transparent);",
        size * 0.08,
        size * 0.08,
        size * 0.06
    );
    rsx! {
        div { class: "{outer_class}", style: "{outer}",
            for style in lobe_styles.iter() { div { class: "{lobe_class}", style: "{style}" } }
            div { class: "{node_class}", style: "{node}" }
        }
    }
}

impl DioxusPatternRenderer for OrbitalVisualizationRenderer {
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
        let complexity = if props.get("complexity").is_some() {
            get_number_or(props, "complexity", 0.0)
        } else {
            calculate_complexity(props.get("schema"))
        };
        let config = config_for(complexity);
        let pixel_size = match get_string_or(props, "size", "md").as_str() {
            "sm" => 120.0,
            "lg" => 300.0,
            "xl" => 400.0,
            _ => 200.0,
        };
        let show_label = get_bool_or(props, "showLabel", true);
        let animated = get_bool_or(props, "animated", true);
        let on_click = get_string(props, "onClick");
        let clickable = on_click.is_some();
        let class_name = get_string_or(props, "className", "");
        let root_class = cn(&[
            BOX,
            "relative flex flex-col items-center justify-center",
            &class_name,
        ]);
        let root_style = format!(
            "width:{pixel_size}px;height:{}px;",
            pixel_size + if show_label { 60.0 } else { 0.0 }
        );
        let inner_class = cn(&[BOX, "relative"]);
        let inner_style = format!(
            "width:{pixel_size}px;height:{pixel_size}px;perspective:{}px;cursor:{};",
            pixel_size * 2.0,
            if clickable { "pointer" } else { "default" }
        );
        let glow_class = cn(&[BOX, "absolute rounded-full opacity-30"]);
        let glow_style = format!(
            "width:{}px;height:{}px;left:50%;top:50%;transform:translate(-50%, -50%);background:radial-gradient(circle, {} 0%, transparent 70%);filter:blur(20px);",
            pixel_size * 0.8,
            pixel_size * 0.8,
            glow(config.series)
        );
        let shape = match config.lobes {
            2 => rsx! {
                {dumbbell(&config, pixel_size, animated, 0.0)}
                if config.has_node {
                    {dumbbell(&config, pixel_size * 0.7, animated, 60.0)}
                    {dumbbell(&config, pixel_size * 0.7, animated, 120.0)}
                }
            },
            4 | 6 => cloverleaf(&config, pixel_size, animated),
            _ => sphere(&config, pixel_size, animated),
        };
        let label_class = cn(&[BOX, "mt-4 text-center"]);
        let title = pattern_spec(
            "typography",
            [
                ("variant", Value::string("body")),
                (
                    "className",
                    Value::string("text-lg font-semibold text-foreground"),
                ),
                ("content", Value::string(config.name)),
            ],
        );
        let sub = pattern_spec(
            "typography",
            [
                ("variant", Value::string("small")),
                ("color", Value::string("muted")),
                (
                    "content",
                    Value::string(format!("Complexity: {complexity} units")),
                ),
            ],
        );
        let sink = view.sink;
        let role = clickable.then_some("button");
        let tabindex = clickable.then_some("0");
        rsx! {
            div { class: "{root_class}", style: "{root_style}", role: role, tabindex: tabindex,
                onclick: move |_| { if let (Some(sink), Some(event)) = (sink, on_click.clone()) { sink.call((event, Value::Null)); } },
                div { class: "{inner_class}", style: "{inner_style}",
                    div { class: "{glow_class}", style: "{glow_style}" }
                    {shape}
                }
                if show_label {
                    div { class: "{label_class}",
                        {registry.render_spec(&title, None, view)}
                        {registry.render_spec(&sub, None, view)}
                    }
                }
                style { "@keyframes orbital-pulse {{ 0%, 100% {{ transform: translate(-50%, -50%) scale(1); opacity: 1; }} 50% {{ transform: translate(-50%, -50%) scale(1.05); opacity: 0.9; }} }} @keyframes orbital-rotate {{ from {{ transform: translate(-50%, -50%) rotate(0deg); }} to {{ transform: translate(-50%, -50%) rotate(360deg); }} }}" }
            }
        }
    }
}
