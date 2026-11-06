//! Dioxus renderer for the `map-view` pattern: the web's container, markers and routes over a Web Mercator projection.
//! The OpenStreetMap tile layer, popups and click-to-coordinate need a map engine and are not drawn.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, object_value, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `map-view` pattern.
pub struct MapViewRenderer;

impl MapViewRenderer {
    const NAME: &str = "map-view";
}

const MARKER_ICON: &str = "https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png";

fn world_px(lat: f64, lng: f64, zoom: f64) -> (f64, f64) {
    let scale = 256.0 * 2f64.powf(zoom);
    let x = (lng + 180.0) / 360.0 * scale;
    let sin = lat.to_radians().sin();
    let y = (0.5 - ((1.0 + sin) / (1.0 - sin)).ln() / (4.0 * std::f64::consts::PI)) * scale;
    (x, y)
}

struct Pin {
    dx: f64,
    dy: f64,
    marker: Value,
}

struct Line {
    points: String,
    color: String,
    weight: f64,
    opacity: f64,
    dash: Option<String>,
}

impl DioxusPatternRenderer for MapViewRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let center_lat = get_number_or(props, "centerLat", 51.505);
        let center_lng = get_number_or(props, "centerLng", -0.09);
        let zoom = get_number_or(props, "zoom", 13.0);
        let height = get_string_or(props, "height", "400px");
        let surface = if get_string_or(props, "surface", "auto") == "auto" {
            "surface-content"
        } else {
            ""
        };
        let class_name = get_string_or(props, "className", "");
        let class = cn(&[
            "bg-transparent rounded-none shadow-none",
            "relative isolate z-0 w-full overflow-hidden rounded-container",
            surface,
            class_name.as_str(),
        ]);
        let (cx, cy) = world_px(center_lat, center_lng, zoom);
        let pins: Vec<Pin> = get_array(props, "markers")
            .into_iter()
            .map(|marker| {
                let (x, y) = world_px(
                    get_number_or(&marker, "lat", 0.0),
                    get_number_or(&marker, "lng", 0.0),
                    zoom,
                );
                Pin {
                    dx: x - cx,
                    dy: y - cy,
                    marker,
                }
            })
            .collect();
        let lines: Vec<Line> = get_array(props, "routes")
            .into_iter()
            .map(|route| Line {
                points: get_array(&route, "waypoints")
                    .iter()
                    .map(|waypoint| {
                        let (x, y) = world_px(
                            get_number_or(waypoint, "lat", 0.0),
                            get_number_or(waypoint, "lng", 0.0),
                            zoom,
                        );
                        format!("{},{}", x - cx, y - cy)
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                color: get_string_or(&route, "color", "var(--color-primary)"),
                weight: get_number_or(&route, "weight", 4.0),
                opacity: get_number_or(&route, "opacity", 0.8),
                dash: get_string(&route, "dashArray"),
            })
            .collect();
        let marker_event = get_string(props, "markerClickEvent");
        let interactive = marker_event.is_some();
        let sink = view.sink;
        let show_attribution = get_bool_or(props, "showAttribution", true);
        let canvas = "position:relative;overflow:hidden;height:100%;width:100%;background:#ddd;outline-offset:1px;font-family:'Helvetica Neue',Arial,Helvetica,sans-serif;font-size:12px;line-height:1.5";

        rsx! {
            div { class: "{class}", style: "height:{height}", "data-testid": "map-view",
                div { class: "leaflet-container", style: "{canvas}",
                    svg { style: "position:absolute;left:50%;top:50%;width:1px;height:1px;overflow:visible",
                        for line in lines.iter() {
                            polyline {
                                points: "{line.points}",
                                fill: "none",
                                stroke: "{line.color}",
                                stroke_width: "{line.weight}",
                                stroke_opacity: "{line.opacity}",
                                stroke_dasharray: line.dash.clone(),
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                            }
                        }
                    }
                    for pin in pins.iter() {
                        {
                            let left = format!("calc(50% + {}px - 12px)", pin.dx);
                            let top = format!("calc(50% + {}px - 41px)", pin.dy);
                            let marker = pin.marker.clone();
                            let event = marker_event.clone();
                            let cursor = if interactive { "cursor:pointer;" } else { "" };
                            rsx! {
                                img {
                                    class: "leaflet-marker-icon",
                                    src: MARKER_ICON,
                                    alt: "Marker",
                                    style: "position:absolute;width:25px;height:41px;left:{left};top:{top};{cursor}",
                                    onclick: move |_| {
                                        if let (Some(sink), Some(event)) = (sink, &event) {
                                            let payload = match &marker {
                                                Value::Object(_) => marker.clone(),
                                                _ => object_value::<&str>([]),
                                            };
                                            sink.call((event.clone(), payload));
                                        }
                                    },
                                }
                            }
                        }
                    }
                    div { class: "leaflet-top leaflet-left", style: "position:absolute;z-index:1000;top:0;left:0;pointer-events:none",
                        div { class: "leaflet-control-zoom leaflet-bar leaflet-control", style: "margin-top:10px;margin-left:10px;pointer-events:auto;box-shadow:0 1px 5px rgba(0,0,0,0.65);border-radius:4px;background:#fff",
                            a { class: "leaflet-control-zoom-in", role: "button", title: "Zoom in", "aria-label": "Zoom in", style: "display:block;width:30px;height:30px;line-height:30px;text-align:center;text-decoration:none;color:black;background:#fff;border-bottom:1px solid #ccc;border-radius:2px 2px 0 0;font:bold 18px 'Lucida Console',Monaco,monospace", "+" }
                            a { class: "leaflet-control-zoom-out", role: "button", title: "Zoom out", "aria-label": "Zoom out", style: "display:block;width:30px;height:30px;line-height:30px;text-align:center;text-decoration:none;color:black;background:#fff;border-radius:0 0 2px 2px;font:bold 18px 'Lucida Console',Monaco,monospace", "\u{2212}" }
                        }
                    }
                    if show_attribution {
                        div { style: "position:absolute;right:0;bottom:0;padding:0 5px;background:rgba(255,255,255,0.8);color:#333;",
                            a { href: "https://leafletjs.com", title: "A JavaScript library for interactive maps", "Leaflet" }
                            " | © "
                            a { href: "https://www.openstreetmap.org/copyright", "OpenStreetMap" }
                        }
                    }
                }
            }
        }
    }
}
