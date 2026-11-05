//! Native Dioxus body for the `graph-view` pattern, ported from web `GraphView.tsx`.
//! Layout math (force simulation, flow/tree/radial) is a port of web `lib/graphViewLayouts.ts`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{object_value, DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::HashMap;

pub struct GraphViewRenderer;

impl GraphViewRenderer {
    const NAME: &str = "graph-view";
}

#[derive(Props, Clone, PartialEq)]
struct GraphProps {
    props: Value,
    view: OwnedDioxusView,
}

const DEFAULT_NODE_COLOR: &str = "var(--color-muted-foreground)";
const DEFAULT_EDGE_COLOR: &str = "var(--color-border)";
const DEFAULT_NODE_SIZE: f64 = 8.0;
const EDGE_ARROW_GAP: f64 = 3.0;

#[derive(Clone)]
struct SimNode {
    id: String,
    label: Option<String>,
    color: String,
    size: f64,
    group: Option<String>,
    x: f64,
    y: f64,
}

struct Adjacency {
    out: Vec<Vec<usize>>,
    inc: Vec<Vec<usize>>,
}

struct DrawnEdge {
    ends: (f64, f64, f64, f64),
    color: String,
    opacity: f64,
    label_opacity: f64,
    label: Option<(f64, f64, String)>,
}

fn num(value: f64) -> String {
    format!("{value}")
}

fn text_field(row: &Value, key: &str) -> Option<String> {
    match row.get(key) {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Int(n)) => Some(n.to_string()),
        Some(Value::Number(n)) => Some(num(*n)),
        _ => None,
    }
}

fn adjacency(ids: &[String], edges: &[(String, String)]) -> Adjacency {
    let index: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut out = vec![vec![]; ids.len()];
    let mut inc = vec![vec![]; ids.len()];
    for (source, target) in edges {
        if let (Some(&s), Some(&t)) = (index.get(source.as_str()), index.get(target.as_str())) {
            out[s].push(t);
            inc[t].push(s);
        }
    }
    Adjacency { out, inc }
}

fn assign_layers(n: usize, adj: &Adjacency, roots: &[usize]) -> Vec<usize> {
    fn visit(id: usize, adj: &Adjacency, layer: &mut Vec<usize>, on_stack: &mut Vec<bool>) {
        on_stack[id] = true;
        let current = layer[id];
        for &next in &adj.out[id] {
            if on_stack[next] {
                continue;
            }
            if current + 1 > layer[next] {
                layer[next] = current + 1;
            }
            visit(next, adj, layer, on_stack);
        }
        on_stack[id] = false;
    }
    let mut layer = vec![0; n];
    let mut on_stack = vec![false; n];
    for &root in roots {
        visit(root, adj, &mut layer, &mut on_stack);
    }
    layer
}

fn visit_order(n: usize, adj: &Adjacency, roots: &[usize]) -> Vec<usize> {
    fn visit(id: usize, adj: &Adjacency, order: &mut Vec<Option<usize>>, counter: &mut usize) {
        if order[id].is_some() {
            return;
        }
        order[id] = Some(*counter);
        *counter += 1;
        for &next in &adj.out[id] {
            visit(next, adj, order, counter);
        }
    }
    let mut order = vec![None; n];
    let mut counter = 0;
    for &root in roots {
        visit(root, adj, &mut order, &mut counter);
    }
    for slot in order.iter_mut() {
        if slot.is_none() {
            *slot = Some(counter);
            counter += 1;
        }
    }
    order.into_iter().map(|o| o.unwrap_or(0)).collect()
}

fn bfs_depth_and_order(n: usize, adj: &Adjacency, roots: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let mut depth = vec![0; n];
    let mut order: Vec<Option<usize>> = vec![None; n];
    let mut visited = vec![false; n];
    let mut queue = Vec::new();
    let mut counter = 0;
    for &root in roots {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        order[root] = Some(counter);
        counter += 1;
        queue.push(root);
    }
    let mut head = 0;
    while head < queue.len() {
        let id = queue[head];
        head += 1;
        for &next in &adj.out[id] {
            if visited[next] {
                continue;
            }
            visited[next] = true;
            depth[next] = depth[id] + 1;
            order[next] = Some(counter);
            counter += 1;
            queue.push(next);
        }
    }
    for slot in order.iter_mut() {
        if slot.is_none() {
            *slot = Some(counter);
            counter += 1;
        }
    }
    (depth, order.into_iter().map(|o| o.unwrap_or(0)).collect())
}

fn edge_walk_order(n: usize, adj: &Adjacency) -> Vec<usize> {
    let mut visited = vec![false; n];
    let mut result = Vec::new();
    for start in 0..n {
        if visited[start] {
            continue;
        }
        let mut current = Some(start);
        while let Some(id) = current {
            if visited[id] {
                break;
            }
            visited[id] = true;
            result.push(id);
            current = adj.out[id].iter().copied().find(|&next| !visited[next]);
        }
    }
    result
}

fn group_by_tier(n: usize, tier: &[usize], max_tier: usize) -> Vec<Vec<usize>> {
    let mut groups = vec![vec![]; max_tier + 1];
    for id in 0..n {
        groups[tier[id]].push(id);
    }
    groups
}

fn distribute_axis(count: usize, start: f64, end: f64) -> Vec<f64> {
    if count == 0 {
        return vec![];
    }
    let slot = (end - start) / count as f64;
    (0..count)
        .map(|i| start + slot * (i as f64 + 0.5))
        .collect()
}

fn static_layout(
    mode: &str,
    ids: &[String],
    edges: &[(String, String)],
    width: f64,
    height: f64,
) -> Vec<(f64, f64)> {
    let margin = 40.0;
    let n = ids.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(width / 2.0, height / 2.0)];
    }
    let adj = adjacency(ids, edges);
    let roots: Vec<usize> = (0..n).filter(|&i| adj.inc[i].is_empty()).collect();
    let mut positions = vec![(0.0, 0.0); n];
    match mode {
        "flow" => {
            let layers = assign_layers(n, &adj, &roots);
            let max_layer = layers.iter().copied().max().unwrap_or(0);
            let groups = group_by_tier(n, &layers, max_layer);
            let mut y_by_id: Vec<Option<f64>> = vec![None; n];
            for (l, group) in groups.iter().enumerate() {
                let x = margin + (l as f64 * (width - 2.0 * margin)) / (max_layer.max(1) as f64);
                let mut keyed: Vec<(usize, f64)> = group
                    .iter()
                    .map(|&id| {
                        let parents: Vec<f64> =
                            adj.inc[id].iter().filter_map(|&p| y_by_id[p]).collect();
                        let avg = if parents.is_empty() {
                            f64::INFINITY
                        } else {
                            parents.iter().sum::<f64>() / parents.len() as f64
                        };
                        (id, avg)
                    })
                    .collect();
                keyed.sort_by(|a, b| {
                    if a.1 != b.1 {
                        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.0.cmp(&b.0)
                    }
                });
                let ys = distribute_axis(keyed.len(), margin, height - margin);
                for (i, (id, _)) in keyed.iter().enumerate() {
                    positions[*id] = (x, ys[i]);
                    y_by_id[*id] = Some(ys[i]);
                }
            }
        }
        "tree" => {
            let effective: Vec<usize> = if roots.is_empty() {
                vec![0]
            } else {
                roots.clone()
            };
            let layers = assign_layers(n, &adj, &effective);
            let max_layer = layers.iter().copied().max().unwrap_or(0);
            let order = visit_order(n, &adj, &effective);
            let groups = group_by_tier(n, &layers, max_layer);
            for (l, group) in groups.iter().enumerate() {
                let mut ordered = group.clone();
                ordered.sort_by_key(|&id| order[id]);
                let y = margin + (l as f64 * (height - 2.0 * margin)) / (max_layer.max(1) as f64);
                let xs = distribute_axis(ordered.len(), margin, width - margin);
                for (i, id) in ordered.iter().enumerate() {
                    positions[*id] = (xs[i], y);
                }
            }
        }
        _ => {
            let cx = width / 2.0;
            let cy = height / 2.0;
            let max_radius = width.min(height) / 2.0 - margin;
            if roots.is_empty() {
                let order = edge_walk_order(n, &adj);
                let k = order.len() as f64;
                for (i, id) in order.iter().enumerate() {
                    let angle = (i as f64 / k) * 2.0 * std::f64::consts::PI;
                    positions[*id] = (cx + max_radius * angle.cos(), cy + max_radius * angle.sin());
                }
            } else {
                let (depth, order) = bfs_depth_and_order(n, &adj, &roots);
                let max_depth = depth.iter().copied().max().unwrap_or(0);
                let groups = group_by_tier(n, &depth, max_depth);
                for (d, group) in groups.iter().enumerate() {
                    let mut ordered = group.clone();
                    ordered.sort_by_key(|&id| order[id]);
                    let radius = (d as f64 * max_radius) / (max_depth.max(1) as f64);
                    let k = ordered.len() as f64;
                    for (i, id) in ordered.iter().enumerate() {
                        let angle = (i as f64 / k) * 2.0 * std::f64::consts::PI;
                        positions[*id] = (cx + radius * angle.cos(), cy + radius * angle.sin());
                    }
                }
            }
        }
    }
    positions
}

fn force_layout(nodes: &mut [SimNode], edges: &[(String, String)], width: f64, height: f64) {
    let n = nodes.len();
    let radius = width.min(height) * 0.3;
    for (idx, node) in nodes.iter_mut().enumerate() {
        let angle = (idx as f64 / n as f64) * 2.0 * std::f64::consts::PI;
        node.x = width / 2.0 + radius * angle.cos();
        node.y = height / 2.0 + radius * angle.sin();
    }
    let mut vx = vec![0.0; n];
    let mut vy = vec![0.0; n];
    let pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|(s, t)| {
            let si = nodes.iter().position(|node| &node.id == s)?;
            let ti = nodes.iter().position(|node| &node.id == t)?;
            Some((si, ti))
        })
        .collect();
    for _ in 0..120 {
        let mut fx = vec![0.0; n];
        let mut fy = vec![0.0; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = nodes[j].x - nodes[i].x;
                let dy = nodes[j].y - nodes[i].y;
                let raw = (dx * dx + dy * dy).sqrt();
                let dist = if raw == 0.0 { 1.0 } else { raw };
                let force = 800.0 / (dist * dist);
                let ffx = (dx / dist) * force;
                let ffy = (dy / dist) * force;
                fx[i] -= ffx;
                fy[i] -= ffy;
                fx[j] += ffx;
                fy[j] += ffy;
            }
        }
        for &(s, t) in &pairs {
            let dx = nodes[t].x - nodes[s].x;
            let dy = nodes[t].y - nodes[s].y;
            let raw = (dx * dx + dy * dy).sqrt();
            let dist = if raw == 0.0 { 1.0 } else { raw };
            let force = (dist - 100.0) * 0.05;
            let ffx = (dx / dist) * force;
            let ffy = (dy / dist) * force;
            fx[s] += ffx;
            fy[s] += ffy;
            fx[t] -= ffx;
            fy[t] -= ffy;
        }
        for i in 0..n {
            fx[i] += (width / 2.0 - nodes[i].x) * 0.01;
            fy[i] += (height / 2.0 - nodes[i].y) * 0.01;
        }
        let damping = 0.85;
        let margin = 40.0;
        for i in 0..n {
            vx[i] = (vx[i] + fx[i]) * damping;
            vy[i] = (vy[i] + fy[i]) * damping;
            nodes[i].x = (nodes[i].x + vx[i]).min(width - margin).max(margin);
            nodes[i].y = (nodes[i].y + vy[i]).min(height - margin).max(margin);
        }
    }
}

fn edge_endpoints(source: &SimNode, target: &SimNode) -> (f64, f64, f64, f64) {
    let dx = target.x - source.x;
    let dy = target.y - source.y;
    let d = dx.hypot(dy);
    if d <= source.size + target.size + EDGE_ARROW_GAP {
        return (source.x, source.y, target.x, target.y);
    }
    let ux = dx / d;
    let uy = dy / d;
    (
        source.x + ux * source.size,
        source.y + uy * source.size,
        target.x - ux * (target.size + EDGE_ARROW_GAP),
        target.y - uy * (target.size + EDGE_ARROW_GAP),
    )
}

fn node_payload(node: &SimNode) -> Value {
    let mut fields = vec![
        ("id", Value::string(node.id.clone())),
        ("color", Value::string(node.color.clone())),
        ("size", Value::Number(node.size)),
    ];
    if let Some(label) = &node.label {
        fields.push(("label", Value::string(label.clone())));
    }
    if let Some(group) = &node.group {
        fields.push(("group", Value::string(group.clone())));
    }
    object_value(fields)
}

impl DioxusPatternRenderer for GraphViewRenderer {
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
        rsx! { GraphViewCanvas { props: props.clone(), view: view.to_owned() } }
    }
}

#[allow(non_snake_case)]
fn GraphViewCanvas(input: GraphProps) -> Element {
    let props = &input.props;
    let sink = input.view.sink;
    let mut hovered = use_signal(|| None::<String>);

    let prop_width = get_number(props, "width");
    let w = prop_width.unwrap_or(600.0);
    let h = get_number(props, "height").unwrap_or(400.0);
    let class_name = get_string_or(props, "className", "");
    let show_labels = get_bool_or(props, "showLabels", true);
    let zoom_to_fit = get_bool_or(props, "zoomToFit", true);
    let layout = get_string_or(props, "layout", "force");
    let on_click = get_string(props, "onNodeClick");
    let on_hover = get_string(props, "onNodeHover");

    let node_rows = get_array(props, "nodes");
    if node_rows.is_empty() {
        let class = format!("flex items-center justify-center {class_name}");
        let style = format!("width:{}px;height:{}px", num(w), num(h));
        return rsx! { div { class: "{class}", style: "{style}", div { class: "text-muted-foreground text-sm", "No graph data" } } };
    }

    let mut groups: Vec<String> = Vec::new();
    for row in &node_rows {
        if let Some(group) = text_field(row, "group").filter(|g| !g.is_empty()) {
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
    }
    let mut nodes: Vec<SimNode> = node_rows
        .iter()
        .map(|row| {
            let group = text_field(row, "group").filter(|g| !g.is_empty());
            let color = match (get_string(row, "color").filter(|c| !c.is_empty()), &group) {
                (Some(color), _) => color,
                (None, Some(group)) => {
                    let idx = groups.iter().position(|g| g == group).unwrap_or(0);
                    format!("var(--color-series-{})", idx % 8 + 1)
                }
                (None, None) => DEFAULT_NODE_COLOR.to_string(),
            };
            SimNode {
                id: text_field(row, "id").unwrap_or_default(),
                label: text_field(row, "label"),
                color,
                size: get_number(row, "size").unwrap_or(DEFAULT_NODE_SIZE),
                group,
                x: 0.0,
                y: 0.0,
            }
        })
        .collect();
    let edge_rows = get_array(props, "edges");
    let edges: Vec<(String, String)> = edge_rows
        .iter()
        .map(|e| {
            (
                text_field(e, "source").unwrap_or_default(),
                text_field(e, "target").unwrap_or_default(),
            )
        })
        .collect();

    if layout == "force" {
        force_layout(&mut nodes, &edges, w, h);
    } else {
        let ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let points = static_layout(&layout, &ids, &edges, w, h);
        for (node, (x, y)) in nodes.iter_mut().zip(points) {
            node.x = x;
            node.y = y;
        }
    }

    let view_box = if zoom_to_fit {
        let pad = 60.0;
        let min_x = nodes
            .iter()
            .map(|n| n.x - n.size)
            .fold(f64::INFINITY, f64::min);
        let min_y = nodes
            .iter()
            .map(|n| n.y - n.size)
            .fold(f64::INFINITY, f64::min);
        let max_x = nodes
            .iter()
            .map(|n| n.x + n.size)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = nodes
            .iter()
            .map(|n| n.y + n.size)
            .fold(f64::NEG_INFINITY, f64::max);
        format!(
            "{} {} {} {}",
            num(min_x - pad),
            num(min_y - pad),
            num(max_x - min_x + pad * 2.0),
            num(max_y - min_y + pad * 2.0)
        )
    } else {
        format!("0 0 {} {}", num(w), num(h))
    };

    let directed = layout != "force";
    let marker_id = {
        let mut hash: u64 = 0xcbf29ce484222325;
        for node in &nodes {
            for byte in node.id.bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        format!("graph-view-arrow-{hash:x}")
    };
    let marker_ref = format!("url(#{marker_id})");

    let hovered_id = hovered();
    let connected: Vec<String> = match &hovered_id {
        Some(id) => {
            let mut set = vec![id.clone()];
            for (s, t) in &edges {
                if s == id {
                    set.push(t.clone());
                }
                if t == id {
                    set.push(s.clone());
                }
            }
            set
        }
        None => vec![],
    };
    let is_connected = |id: &str| hovered_id.is_none() || connected.iter().any(|c| c == id);

    let drawn: Vec<DrawnEdge> = edge_rows
        .iter()
        .zip(edges.iter())
        .filter_map(|(row, (s, t))| {
            let source = nodes.iter().find(|n| &n.id == s)?;
            let target = nodes.iter().find(|n| &n.id == t)?;
            let highlighted = hovered_id.is_none() || (is_connected(s) && is_connected(t));
            let ends = if directed {
                edge_endpoints(source, target)
            } else {
                (source.x, source.y, target.x, target.y)
            };
            Some(DrawnEdge {
                ends,
                color: get_string(row, "color")
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| DEFAULT_EDGE_COLOR.to_string()),
                opacity: if highlighted { 0.8 } else { 0.15 },
                label_opacity: if highlighted { 0.9 } else { 0.2 },
                label: text_field(row, "label").filter(|l| !l.is_empty()).map(|l| {
                    (
                        (source.x + target.x) / 2.0,
                        (source.y + target.y) / 2.0 - 6.0,
                        l,
                    )
                }),
            })
        })
        .collect();

    let role = if on_click.is_some() { "group" } else { "img" };
    let aria = get_string(props, "aria-label").unwrap_or_else(|| "Graph".to_string());
    let root_class = format!("relative overflow-hidden {class_name}");
    let root_style = format!(
        "width:{};height:{}px",
        match prop_width {
            Some(width) => format!("{}px", num(width)),
            None => "100%".to_string(),
        },
        num(h)
    );
    let clickable = on_click.is_some();

    rsx! {
        div { class: "{root_class}", role: "{role}", "aria-label": "{aria}", style: "{root_style}",
            svg { width: "100%", height: "100%", view_box: "{view_box}", preserve_aspect_ratio: "xMidYMid meet",
                if directed {
                    defs {
                        marker {
                            id: "{marker_id}",
                            view_box: "0 0 10 10",
                            ref_x: "9",
                            ref_y: "5",
                            marker_width: "6",
                            marker_height: "6",
                            orient: "auto-start-reverse",
                            path { d: "M0,0 L10,5 L0,10 z", fill: "currentColor" }
                        }
                    }
                }
                for edge in drawn.iter() {
                    g {
                        line {
                            x1: num(edge.ends.0),
                            y1: num(edge.ends.1),
                            x2: num(edge.ends.2),
                            y2: num(edge.ends.3),
                            stroke: "{edge.color}",
                            color: "{edge.color}",
                            stroke_width: "1.5",
                            opacity: num(edge.opacity),
                            marker_end: if directed { Some(marker_ref.clone()) } else { None },
                        }
                        if show_labels {
                            if let Some((lx, ly, label)) = edge.label.as_ref() {
                                text {
                                    x: num(*lx),
                                    y: num(*ly),
                                    text_anchor: "middle",
                                    fill: "var(--color-muted-foreground)",
                                    font_size: "9",
                                    opacity: num(edge.label_opacity),
                                    "{label}"
                                }
                            }
                        }
                    }
                }
                for node in nodes.iter() {
                    {
                        let is_hovered = hovered_id.as_deref() == Some(node.id.as_str());
                        let highlighted = is_connected(&node.id);
                        let r = if is_hovered { node.size * 1.4 } else { node.size };
                        let label = node.label.clone().unwrap_or_else(|| node.id.clone());
                        let aria_node = clickable.then(|| match &node.group {
                            Some(group) => format!("{label}, {group}"),
                            None => label.clone(),
                        });
                        let enter_id = node.id.clone();
                        let click_payload = node_payload(node);
                        let hover_payload = node_payload(node);
                        let click_event = on_click.clone();
                        let hover_event = on_hover.clone();
                        let leave_event = on_hover.clone();
                        let cursor = if clickable { "cursor: pointer" } else { "cursor: default" };
                        rsx! {
                            g {
                                style: "{cursor}",
                                "aria-label": aria_node,
                                onmouseenter: move |_| {
                                    hovered.set(Some(enter_id.clone()));
                                    if let (Some(s), Some(e)) = (sink, hover_event.clone()) {
                                        s.call((e, hover_payload.clone()));
                                    }
                                },
                                onmouseleave: move |_| {
                                    hovered.set(None);
                                    if let (Some(s), Some(e)) = (sink, leave_event.clone()) {
                                        s.call((e, Value::Null));
                                    }
                                },
                                onclick: move |_| {
                                    if let (Some(s), Some(e)) = (sink, click_event.clone()) {
                                        s.call((e, click_payload.clone()));
                                    }
                                },
                                circle {
                                    cx: num(node.x),
                                    cy: num(node.y),
                                    r: num(r),
                                    fill: "{node.color}",
                                    opacity: if highlighted { "1" } else { "0.3" },
                                    stroke: if is_hovered { "var(--color-background)" } else { "none" },
                                    stroke_width: if is_hovered { "2" } else { "0" },
                                }
                                if show_labels {
                                    text {
                                        x: num(node.x),
                                        y: num(node.y + node.size + 12.0),
                                        text_anchor: "middle",
                                        fill: "var(--color-foreground)",
                                        font_size: "11",
                                        font_weight: if is_hovered { "bold" } else { "normal" },
                                        opacity: if highlighted { "0.9" } else { "0.2" },
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
