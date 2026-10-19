//! Native behavior catalog. One winit window, built
//! with the same dioxus primitives the renderer itself uses: a sidebar of
//! every baked behavior (the SAME gzipped corpus `orb verify` resolves from —
//! no filesystem paths), and a content pane that mounts the selected
//! behavior's `.orb` through `shell::mount_core` and paints it with the
//! existing root [`App`]. Event chips extracted from the schema drive the
//! state machine (`shell::drive_event`), so this is also the native parity
//! harness for the ui-*/std-* sweep.

#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use dioxus::prelude::*;
use std::sync::OnceLock;

use crate::app::{poke_rerender, App};
use crate::shell;

/// Discovers the installed `@almadar/std` package and reads its `.orb`
/// registry straight off disk — the native catalog's replacement for the
/// deleted `EMBEDDED_STD_BEHAVIORS` phf map (Phase 2,
/// `docs/Almadar_Behavior_Packages.md` §10: behaviors are installed npm data,
/// never baked into the binary). Exposed at crate visibility so
/// `bin/orbital-native-swapstress.rs` (a separate binary target linking this
/// crate's lib) shares the same discovery/read path.
pub mod std_registry {
    use orbital_core::behavior_packages::{discover, user_store_dir, BehaviorPackage};
    use std::path::{Path, PathBuf};

    /// The installed `almadar-std`-prefixed package resolved from the
    /// working directory's walk (+ the user store as the final rung), or
    /// `None` when no std package is installed/discoverable.
    pub fn std_package() -> Option<BehaviorPackage> {
        let cwd = std::env::current_dir().ok()?;
        let user_store = user_store_dir().unwrap_or_else(|| cwd.clone());
        let packages = discover(&cwd, &user_store).ok()?;
        packages.into_iter().find(|p| p.prefix == "almadar-std")
    }

    /// Every behavior file under the installed std's registry (`.orb` for a
    /// materialized package, `.lolo` for a live one): the canonical
    /// `std/behaviors/<stem>` import form paired with its real path on disk.
    pub fn std_registry_rows(pkg: &BehaviorPackage) -> Vec<(String, PathBuf)> {
        let mut rows = Vec::new();
        let extension = pkg.registry.extension();
        let mut stack = vec![pkg.registry.root().to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some(extension) {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                rows.push((format!("std/behaviors/{stem}"), path));
            }
        }
        rows
    }

    /// A behavior file's program, resolved by `orb`.
    pub fn read_orb(path: &Path) -> Result<String, String> {
        crate::shell::orb_resolve(path)
    }
}

/// One sidebar row: behavior name (as authored), composition level + topic
/// derived from the registry directory it was found under (`<topic…>/
/// {atoms,molecules,organisms,templates}/<name>.orb`), and the real path to
/// read its `.orb` content from.
struct CatalogRow {
    name: String,
    level: String,
    topic: String,
    file: std::path::PathBuf,
}

/// Tier display order + plural labels.
const TIER_ORDER: &[&str] = &["atom", "molecule", "organism", "template"];

fn tier_label(level: &str) -> String {
    match level {
        "atom" => "Atoms".to_string(),
        "molecule" => "Molecules".to_string(),
        "organism" => "Organisms".to_string(),
        "template" => "Templates".to_string(),
        other => other.to_string(),
    }
}

fn tier_rank(level: &str) -> usize {
    TIER_ORDER
        .iter()
        .position(|t| *t == level)
        .unwrap_or(TIER_ORDER.len())
}

/// Theme names parsed from the injected CSS bundle (`[data-theme="…"]`
/// selectors in `almadar-theme.css`) — the catalog header's theme cycler.
fn theme_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = std::collections::BTreeSet::new();
        crate::app::CSS_TEXT.with(|c| {
            for css in c.borrow().iter() {
                let mut rest = css.as_str();
                while let Some(i) = rest.find("[data-theme=\"") {
                    rest = &rest[i + "[data-theme=\"".len()..];
                    let Some(j) = rest.find('"') else { break };
                    let name = &rest[..j];
                    if name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b == b'-' || b.is_ascii_digit())
                    {
                        names.insert(name.to_string());
                    }
                    rest = &rest[j + 1..];
                }
            }
        });
        names.into_iter().collect()
    })
}

/// Cycle the active theme (0 = fallback) — writes the `THEME` cell the root
/// `App` reads into `data-theme`, then pokes a re-render.
fn cycle_theme(
    theme_idx: &mut Signal<usize>,
    themes: &'static Vec<String>,
    delta: i64,
    rev: &mut Signal<u32>,
) {
    let n = (themes.len() + 1) as i64;
    let next = ((*theme_idx.read() as i64) + delta).rem_euclid(n) as usize;
    theme_idx.set(next);
    crate::app::THEME.with(|t| {
        *t.borrow_mut() = if next == 0 {
            None
        } else {
            themes.get(next - 1).cloned()
        };
    });
    poke_rerender();
    *rev.write() += 1;
}

/// True once `catalog_rows()` has run and found no installed `@almadar/std`
/// package — the sidebar shows an explicit "std not installed" message
/// instead of silently rendering an empty list.
fn std_missing() -> bool {
    static MISSING: OnceLock<bool> = OnceLock::new();
    *MISSING.get_or_init(|| std_registry::std_package().is_none())
}

/// Every `.orb` in the installed std's registry, sorted by (tier, topic,
/// name). Built once from a real filesystem walk of the discovered package —
/// the same source of truth `orb verify` resolves `uses:` from now that the
/// embedded phf corpus is gone (Phase 2). Empty (never panics) when std isn't
/// installed; see `std_missing()`.
fn catalog_rows() -> &'static Vec<CatalogRow> {
    static ROWS: OnceLock<Vec<CatalogRow>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let Some(pkg) = std_registry::std_package() else {
            return Vec::new();
        };
        let mut rows: Vec<CatalogRow> = std_registry::std_registry_rows(&pkg)
            .into_iter()
            .filter_map(|(canonical, file)| {
                let name = canonical.strip_prefix("std/behaviors/")?.to_string();
                let rel = file.strip_prefix(pkg.registry.root()).ok()?;
                let mut dirs: Vec<String> = rel
                    .parent()?
                    .components()
                    .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
                    .collect();
                let tier_plural = dirs.pop().unwrap_or_default();
                let level = match tier_plural.as_str() {
                    "atoms" => "atom",
                    "molecules" => "molecule",
                    "organisms" => "organism",
                    "templates" => "template",
                    other => other,
                }
                .to_string();
                let topic = if dirs.is_empty() {
                    "(root)".to_string()
                } else {
                    dirs.join("/")
                };
                Some(CatalogRow {
                    name,
                    level,
                    topic,
                    file,
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            tier_rank(&a.level)
                .cmp(&tier_rank(&b.level))
                .then(a.topic.cmp(&b.topic))
                .then(a.name.cmp(&b.name))
        });
        rows
    })
}

/// Extract every distinct `"event": "NAME"` string from a schema's transitions
/// — the catalog's event chips. Recursive key walk: the `.orb` shape varies
/// (stateMachine vs flat transitions) and the catalog only needs the names.
fn extract_events(schema_json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(schema_json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, val) in map {
                    if k == "event" {
                        if let Some(s) = val.as_str() {
                            out.push(s.to_string());
                        }
                    }
                    walk(val, out);
                }
            }
            serde_json::Value::Array(items) => {
                for i in items {
                    walk(i, out);
                }
            }
            _ => {}
        }
    }
    walk(&v, &mut out);
    out.sort();
    out.dedup();
    out
}

const SIDEBAR_STYLE: &str = "width: 290px; min-width: 290px; height: 100vh; overflow-y: auto; \
    border-right: 1px solid #e2e2e2; padding: 8px; box-sizing: border-box; \
    font-family: sans-serif; font-size: 13px;";
const MAIN_STYLE: &str = "flex: 1; height: 100vh; display: flex; flex-direction: column; \
    overflow: hidden;";

/// Request a behavior swap. Phase 1: unmount the current circuit so the next
/// render commits an EMPTY document, and hand the schema to the mount effect
/// via `pending`. Phase 2 (the `use_effect` in `CatalogApp`) runs after that
/// empty render has committed and mounts fresh. This gives the long-lived
/// native VDOM the wasm shell's fresh-mount-per-swap semantics — a direct
/// cross-behavior in-place diff corrupted the blitz node graph (stale slab
/// keys → `node_at_path` panic).
fn request_swap(
    name: &str,
    json: &str,
    mut selected: Signal<Option<String>>,
    mut events: Signal<Vec<String>>,
    mut mount_error: Signal<Option<String>>,
    mut pending: Signal<Option<String>>,
) {
    events.set(extract_events(json));
    selected.set(Some(name.to_string()));
    mount_error.set(None);
    shell::unmount();
    pending.set(Some(json.to_string()));
}

#[component]
pub fn CatalogApp() -> Element {
    let selected = use_signal(|| Option::<String>::None);
    let mut filter = use_signal(String::new);
    let mut mount_error = use_signal(|| Option::<String>::None);
    let events = use_signal(Vec::<String>::new);
    let mut rev = use_signal(|| 0u32);
    let mut tier = use_signal(|| "all".to_string());
    let mut theme_idx = use_signal(|| 0usize);
    let mut pending = use_signal(|| Option::<String>::None);

    // Swap phase 2: effects fire after the render's mutations are committed,
    // so by the time the fresh mount's create diff runs, the blitz document
    // is empty and consistent.
    use_effect(move || {
        let json = pending.read().clone();
        if let Some(json) = json {
            if let Err(e) = shell::mount_core(&json) {
                mount_error.set(Some(e));
            }
            pending.set(None);
            poke_rerender();
            *rev.write() += 1;
        }
    });

    // Swap-stress mode (`ORBITAL_CATALOG_AUTOSWAP=1`): cycles the two-phase
    // swap over every behavior from INSIDE the real winit event loop — the
    // faithful repro for crashes the headless harness can't reach (real
    // wakers wake the primitives' async focus/effect tasks between swaps).
    // `Delay` paces swaps on a timer thread so the loop goes idle between
    // iterations.
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    {
        if std::env::var("ORBITAL_CATALOG_AUTOSWAP").is_ok() {
            let names: Vec<(String, std::path::PathBuf)> = catalog_rows()
                .iter()
                .map(|r| (r.name.clone(), r.file.clone()))
                .collect();
            use_future(move || {
                let names = names.clone();
                async move {
                    // Real wall-clock pacing: park on a timer thread that
                    // wakes the winit waker after the delay, so the event
                    // loop goes IDLE between swaps (an immediate re-wake
                    // loop here kept the vdom permanently hot and froze
                    // the window — WM "Not Responding").
                    struct Delay(std::time::Duration, bool);
                    impl std::future::Future for Delay {
                        type Output = ();
                        fn poll(
                            mut self: std::pin::Pin<&mut Self>,
                            cx: &mut std::task::Context<'_>,
                        ) -> std::task::Poll<()> {
                            if self.1 {
                                return std::task::Poll::Ready(());
                            }
                            self.1 = true;
                            let w = cx.waker().clone();
                            let d = self.0;
                            std::thread::spawn(move || {
                                std::thread::sleep(d);
                                w.wake();
                            });
                            std::task::Poll::Pending
                        }
                    }
                    let mut cycle = 0usize;
                    loop {
                        for (name, file) in &names {
                            match std_registry::read_orb(file) {
                                Ok(json) => request_swap(
                                    name,
                                    &json,
                                    selected,
                                    events,
                                    mount_error,
                                    pending,
                                ),
                                Err(e) => {
                                    mount_error.set(Some(e))
                                }
                            }
                            poke_rerender();
                            *rev.write() += 1;
                            Delay(std::time::Duration::from_millis(400), false).await;
                        }
                        cycle += 1;
                        tracing::info!(target: shell::NS, cycle, "autoswap cycle complete");
                    }
                }
            });
        }
    }

    let needle = filter.read().to_lowercase();
    let active_tier = tier.read().clone();
    let rows = catalog_rows();
    let current_state = if selected.read().is_some() {
        shell::current_state_string().unwrap_or_default()
    } else {
        String::new()
    };

    // Filter by search + active tier, then group by (tier, topic) preserving
    // the sorted order — mirrors the browser catalog's `tier / topic` headers.
    let filtered: Vec<&CatalogRow> = rows
        .iter()
        .filter(|r| active_tier == "all" || r.level == active_tier)
        .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
        .collect();
    let mut groups: Vec<(String, Vec<&CatalogRow>)> = Vec::new();
    for row in &filtered {
        let key = format!("{} · {}", tier_label(&row.level), row.topic);
        match groups.last_mut() {
            Some((k, items)) if *k == key => items.push(*row),
            _ => groups.push((key, vec![*row])),
        }
    }

    rsx! {
        div { style: "display: flex; width: 100vw; height: 100vh; margin: 0;",
            // ── Sidebar ────────────────────────────────────────────────
            div { style: "{SIDEBAR_STYLE}",
                div { style: "font-weight: 700; font-size: 15px; margin-bottom: 6px;",
                    "Orbital · Native Catalog"
                }
                input {
                    r#type: "text",
                    placeholder: "filter behaviors…",
                    value: "{filter}",
                    style: "width: 100%; box-sizing: border-box; padding: 6px 8px; margin-bottom: 8px; \
                            border: 1px solid #ccc; border-radius: 6px; font-size: 13px;",
                    oninput: move |e| filter.set(e.value()),
                }
                // Tier chips (All + atom/molecule/organism/template).
                div { style: "display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 8px;",
                    for t in std::iter::once("all").chain(TIER_ORDER.iter().copied()) {
                        {
                            let t_name = t.to_string();
                            let active = active_tier == t;
                            let (bg, fg) = if active { ("#2563eb", "#ffffff") } else { ("#f1f5f9", "#475569") };
                            let label = if t == "all" { "All".to_string() } else { tier_label(t) };
                            rsx! {
                                button {
                                    key: "tier-{t_name}",
                                    style: "padding: 2px 8px; border: none; border-radius: 999px; \
                                            background: {bg}; color: {fg}; cursor: pointer; font-size: 11px;",
                                    onclick: move |_| tier.set(t_name.clone()),
                                    "{label}"
                                }
                            }
                        }
                    }
                }
                if std_missing() {
                    div { style: "color: #b91c1c; margin-bottom: 6px;",
                        "@almadar/std is not installed — run `orb update --global`"
                    }
                } else {
                    div { style: "color: #888; margin-bottom: 6px;",
                        "{filtered.len()} / {rows.len()} behaviors · click to mount"
                    }
                }
                for (header, items) in groups.iter() {
                    div {
                        key: "grp-{header}",
                        style: "font-weight: 700; font-size: 11px; text-transform: uppercase; \
                                letter-spacing: 0.05em; color: #64748b; margin: 10px 0 2px 4px;",
                        "{header}"
                    }
                    for row in items.iter() {
                        {
                            let name = row.name.clone();
                            let file = row.file.clone();
                            let level = row.level.clone();
                            let is_sel = selected.read().as_deref() == Some(name.as_str());
                            let bg = if is_sel { "#dbeafe" } else { "transparent" };
                            rsx! {
                                div {
                                    key: "{name}",
                                    style: "padding: 4px 8px; border-radius: 5px; cursor: pointer; \
                                            background: {bg}; display: flex; justify-content: space-between; gap: 6px;",
                                    onclick: move |_| {
                                        match std_registry::read_orb(&file) {
                                            Ok(json) => {
                                                request_swap(&name, &json, selected, events, mount_error, pending)
                                            }
                                            Err(e) => mount_error.set(Some(e)),
                                        }
                                        poke_rerender();
                                        *rev.write() += 1;
                                    },
                                    span { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{name}" }
                                    span { style: "color: #999; flex-shrink: 0;", "{level}" }
                                }
                            }
                        }
                    }
                }
            }
            // ── Main ───────────────────────────────────────────────────
            div { style: "{MAIN_STYLE}",
                div { style: "padding: 8px 14px; border-bottom: 1px solid #e2e2e2; \
                              font-family: sans-serif; font-size: 13px; display: flex; \
                              align-items: center; gap: 12px; flex-wrap: wrap;",
                    if let Some(name) = selected.read().clone() {
                        strong { "{name}" }
                        span { style: "color: #666;", "state: {current_state}" }
                    } else {
                        span { style: "color: #888;", "select a behavior from the sidebar" }
                    }
                    // Theme cycler — `data-theme` on the App root, same contract
                    // as the React shell's theme switcher. Index 0 = the
                    // `:root:not([data-theme])` fallback (minimalist-light).
                    {
                        let themes = theme_names();
                        let idx = *theme_idx.read();
                        let label = if idx == 0 {
                            "default".to_string()
                        } else {
                            themes.get(idx - 1).cloned().unwrap_or_default()
                        };
                        rsx! {
                            span { style: "margin-left: auto; display: flex; align-items: center; gap: 6px; color: #666;",
                                "theme:"
                                button {
                                    style: "border: 1px solid #cbd5e1; border-radius: 4px; background: #f8fafc; cursor: pointer; padding: 0 6px;",
                                    onclick: move |_| cycle_theme(&mut theme_idx, themes, -1, &mut rev),
                                    "◂"
                                }
                                span { style: "min-width: 130px; text-align: center; color: #111;", "{label}" }
                                button {
                                    style: "border: 1px solid #cbd5e1; border-radius: 4px; background: #f8fafc; cursor: pointer; padding: 0 6px;",
                                    onclick: move |_| cycle_theme(&mut theme_idx, themes, 1, &mut rev),
                                    "▸"
                                }
                            }
                        }
                    }
                    if let Some(err) = mount_error.read().clone() {
                        span { style: "color: #b91c1c;", "{err}" }
                    }
                    // Event chips — drive the mounted state machine.
                    for ev in events.read().iter() {
                        {
                            let ev_name = ev.clone();
                            rsx! {
                                button {
                                    key: "ev-{ev_name}",
                                    style: "padding: 2px 10px; border: 1px solid #cbd5e1; border-radius: 999px; \
                                            background: #f8fafc; cursor: pointer; font-size: 12px;",
                                    onclick: move |_| {
                                        if let Err(e) = shell::drive_event(&ev_name, "") {
                                            mount_error.set(Some(e));
                                        }
                                        poke_rerender();
                                        *rev.write() += 1;
                                    },
                                    "{ev_name}"
                                }
                            }
                        }
                    }
                }
                div { style: "flex: 1; overflow-y: auto; padding: 14px;",
                    // `rev` subscribes this subtree to mount/event pokes so the
                    // header state + App re-read the circuit.
                    span { style: "display: none;", "{rev}" }
                    App {}
                }
            }
        }
    }
}
