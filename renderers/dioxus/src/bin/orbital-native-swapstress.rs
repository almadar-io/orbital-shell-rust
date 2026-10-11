//! Headless swap-stress harness — reproduces (and guards) the blitz
//! `slab: invalid key` crash that hits the native catalog when behaviors are
//! swapped inside one long-lived `VirtualDom`. No window, no GPU: builds the
//! SAME `DioxusDocument` + head-sink the wasm shell uses, then cycles
//! `mount_core` + `poke_rerender` over every baked behavior (optionally
//! filtered), polling the vdom to flush mutations into the blitz document —
//! exactly the native catalog's click path minus winit.
//!
//! Usage: `cargo run --release -p orbital-client-dioxus --features native \
//!         --bin orbital-native-swapstress [-- --filter ui- --rounds 3]`
//! Exit 0 = all swaps clean. A panic is the failure signal (run with
//! `RUST_BACKTRACE=1`).

// Desktop-only, like the sibling bins: on wasm32 the bin builds to an empty `main`.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    desktop::main()
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {

    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use dioxus_core::{provide_context, ScopeId, VirtualDom};
    use dioxus_native::DioxusDocument;
    use orbital_client_dioxus::app::{poke_rerender, CSS_TEXT};
    use orbital_client_dioxus::catalog::{std_registry, CatalogApp};
    use orbital_client_dioxus::shell;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    // Head-element sink (mirrors wasm.rs `WasmHeadSink`): the catalog's `App`
    // emits `document::Style` for the CSS text; on this headless path we forward
    // straight into the blitz document after each poll.

    struct HeadOp {
        name: String,
        attributes: Vec<(String, String)>,
        contents: Option<String>,
    }

    struct StressHeadSink {
        ops: Rc<RefCell<VecDeque<HeadOp>>>,
    }

    impl dioxus_document::Document for StressHeadSink {
        fn eval(&self, js: String) -> dioxus_document::Eval {
            dioxus_document::NoOpDocument.eval(js)
        }

        fn create_head_element(
            &self,
            name: &str,
            attributes: &[(&str, String)],
            contents: Option<String>,
        ) {
            self.ops.borrow_mut().push_back(HeadOp {
                name: name.to_string(),
                attributes: attributes
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                contents,
            });
        }
    }

    pub fn main() {
        let mut filter = String::new();
        let mut rounds: usize = 2;
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--filter" => filter = args.next().unwrap_or_default(),
                "--rounds" => rounds = args.next().and_then(|v| v.parse().ok()).unwrap_or(2),
                _ => {}
            }
        }

        let Some(std_pkg) = std_registry::std_package() else {
            eprintln!(
                "[swapstress] @almadar/std is not installed — run \
             `orb update` (or `orb update --global`)"
            );
            std::process::exit(1);
        };
        let mut names: Vec<(String, std::path::PathBuf)> =
            std_registry::std_registry_rows(&std_pkg)
                .into_iter()
                .filter_map(|(canonical, file)| {
                    let name = canonical.strip_prefix("std/behaviors/")?.to_string();
                    (filter.is_empty() || name.contains(&filter)).then_some((name, file))
                })
                .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        println!(
            "[swapstress] {} behaviors × {} rounds (filter='{}')",
            names.len(),
            rounds,
            filter
        );

        // Mirror the native catalog's CSS contract: two real stylesheets so the
        // root `App` emits `document::Style` head ops on every render (a prime
        // suspect for the slab crash — head mutations are path-based too).
        // `--no-css` drops them for A/B isolation.
        let no_css = std::env::args().any(|a| a == "--no-css");
        CSS_TEXT.with(|c| {
            *c.borrow_mut() = if no_css {
                Vec::new()
            } else {
                let mut texts = Vec::new();
                for rel in [
                    "../packages/almadar-ui-wasm/preview/almadar.css",
                    "../packages/almadar-ui-wasm/dist/tailwind.css",
                ] {
                    if let Ok(t) = std::fs::read_to_string(rel) {
                        texts.push(t);
                    }
                }
                if texts.is_empty() {
                    // Fall back to a small synthetic sheet — the crash path is
                    // about head-node churn, not the rules themselves.
                    texts.push("/* stress */ .almadar-button { color: red; }".to_string());
                }
                texts
            }
        });

        let head_ops: Rc<RefCell<VecDeque<HeadOp>>> = Rc::new(RefCell::new(VecDeque::new()));
        let vdom = VirtualDom::new(CatalogApp);
        vdom.in_scope(ScopeId::ROOT, || {
            provide_context(Rc::new(StressHeadSink {
                ops: head_ops.clone(),
            }) as Rc<dyn dioxus_document::Document>);
        });
        let mut doc = DioxusDocument::new(
            vdom,
            blitz_dom::DocumentConfig {
                viewport: Some(blitz_traits::shell::Viewport::new(
                    1400,
                    900,
                    1.0,
                    blitz_traits::shell::ColorScheme::Light,
                )),
                ..Default::default()
            },
        );
        doc.initial_build();

        use blitz_traits::events::{
            BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent,
        };
        use keyboard_types::Modifiers;
        let click_at = |doc: &mut DioxusDocument, x: f32, y: f32| {
            let ev = |button, buttons| {
                UiEvent::MouseDown(BlitzMouseButtonEvent {
                    x,
                    y,
                    button,
                    buttons,
                    mods: Modifiers::default(),
                })
            };
            doc.handle_ui_event(ev(MouseEventButton::Main, MouseEventButtons::Primary));
            let up = UiEvent::MouseUp(BlitzMouseButtonEvent {
                x,
                y,
                button: MouseEventButton::Main,
                buttons: MouseEventButtons::None,
                mods: Modifiers::default(),
            });
            doc.handle_ui_event(up);
        };
        // Flush vdom work + head ops into the blitz document (what the winit
        // frame does around a paint).
        let settle = |doc: &mut DioxusDocument| {
            for _ in 0..4 {
                if !doc.poll(None) {
                    break;
                }
            }
            while let Some(op) = head_ops.borrow_mut().pop_front() {
                doc.create_head_element(&op.name, &op.attributes, &op.contents);
            }
            doc.resolve(0.0);
        };

        let interact = !std::env::args().any(|a| a == "--no-interact");
        let mut mounted = 0usize;
        for round in 0..rounds {
            for (name, file) in &names {
                let json = match std_registry::read_orb(file) {
                    Ok(json) => json,
                    Err(e) => {
                        println!("[swapstress] resolve failed for {name}: {e}");
                        continue;
                    }
                };
                if let Err(e) = shell::mount_core(&json) {
                    println!("[swapstress] mount_core failed for {name}: {e}");
                    continue;
                }
                poke_rerender();
                settle(&mut doc);
                mounted += 1;
                if mounted % 50 == 0 {
                    println!(
                        "[swapstress] round {} · {mounted} swaps · last={name}",
                        round + 1
                    );
                }

                // Click INTO the mounted content before swapping away — the
                // primitives spawn async focus tasks on click; a task that fires
                // after the next swap removed its node is the suspected
                // slab-invalidation path. Content pane starts at x≈300.
                if interact {
                    for &(cx, cy) in &[
                        (420.0f32, 140.0f32),
                        (420.0, 180.0),
                        (500.0, 140.0),
                        (420.0, 220.0),
                    ] {
                        click_at(&mut doc, cx, cy);
                        settle(&mut doc);
                    }
                }
            }
        }
        println!("[swapstress] direct-swap phase OK — {mounted} swaps, no panic");

        // Phase 2 — faithful click path: drive swaps from INSIDE
        // `handle_ui_event`, exactly like the user's sidebar clicks (signal sets
        // re-enter the vdom mid-event). Scan the sidebar column top→bottom with
        // mousedown+mouseup pairs; every row hit mounts its behavior.
        let mut clicks = 0usize;
        for pass in 0..rounds {
            let mut y = 90.0f32;
            while y < 860.0 {
                click_at(&mut doc, 120.0, y);
                settle(&mut doc);
                clicks += 1;
                y += 12.0;
            }
            println!(
                "[swapstress] click pass {} done ({} clicks so far)",
                pass + 1,
                clicks
            );
        }
        println!("[swapstress] OK — {mounted} direct swaps + {clicks} clicks, no panic");
    }
}
