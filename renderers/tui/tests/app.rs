//! The terminal renderer draws the shared client circuit (`ClientCore`).

use orbital_client_tui::{PatternRegistry, TuiApp};
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::OrbitalSchema;
use std::sync::Arc;

fn orbital(name: &str, text: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "entity": { "name": format!("{name}Row"), "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
        "traits": [{
            "name": format!("{name}View"), "linkedEntity": format!("{name}Row"),
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT",
                    "effects": [["render-ui", "main", text]] }]
            }
        }],
        "pages": [{ "name": format!("{name}Home"), "path": format!("/{}", name.to_lowercase()), "traits": [{ "ref": format!("{name}View") }] }]
    })
}

fn app(orbitals: Vec<serde_json::Value>) -> TuiApp {
    let schema: OrbitalSchema = serde_json::from_value(
        serde_json::json!({ "name": "t", "typeContractVersion": 1, "orbitals": orbitals }),
    )
    .unwrap();
    let bridge: Arc<dyn ServerBridge> = Arc::new(KernelBridge::new(schema.clone()).unwrap());
    TuiApp::new(schema, Some(bridge), Vec::new()).unwrap()
}

#[test]
fn the_first_page_mounts_through_the_kernel_and_draws_its_main_slot() {
    let mut app = app(vec![orbital(
        "Notes",
        &serde_json::json!({ "type": "typography", "content": "hello notes" }),
    )]);
    assert!(app
        .core()
        .slot_contents("main")
        .iter()
        .any(|c| c.pattern == "typography"));
    assert!(app.render_to_text(100, 20).unwrap().contains("hello notes"));
}

#[test]
fn control_a_pattern_without_a_terminal_renderer_draws_the_declared_unsupported_answer() {
    let mut app = app(vec![orbital(
        "Notes",
        &serde_json::json!({ "type": "zz-no-such-pattern" }),
    )]);
    assert!(!app.registry().supports("zz-no-such-pattern"));
    let text = app.render_to_text(100, 20).unwrap();
    assert!(!text.contains("No content to display"), "{text}");
}

#[test]
fn the_registry_covers_the_core_patterns() {
    let registry = PatternRegistry::new();
    assert!(
        registry.coverage().len() > 200,
        "{}",
        registry.coverage().len()
    );
    assert!(registry.supports("typography") && registry.supports("button"));
}

#[test]
fn edge_an_empty_main_slot_says_so() {
    let mut app = app(vec![orbital("Notes", &serde_json::json!(null))]);
    assert!(app
        .render_to_text(80, 10)
        .unwrap()
        .contains("No content to display"));
}

#[test]
fn terminal_host_refuses_an_unavailable_ask_through_its_declared_failure_route() {
    use orbital_core::client::client_core::PendingHostAsk;
    use orbital_core::HostAskCompletion;

    let mut note = orbital(
        "Notes",
        &serde_json::json!({ "type": "typography", "content": "ready" }),
    );
    note["traits"][0]["stateMachine"]["transitions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "from": "idle", "to": "idle", "event": "UNAVAILABLE",
            "effects": [["render-ui", "main", { "type": "typography", "content": "audio unavailable" }]]
        }));
    let mut app = app(vec![note]);
    app.core_mut().pending_host_asks.push(PendingHostAsk {
        source_trait: "NotesView".into(),
        op: "audio/play-sound".into(),
        args: vec![orbital_core::Value::string("jump")],
        completion: Some(HostAskCompletion {
            trait_name: "NotesView".into(),
            failure: Some("UNAVAILABLE".into()),
            ..Default::default()
        }),
    });
    app.send_event("INIT", orbital_core::Value::Null);
    assert!(app.core().pending_host_asks.is_empty());
    assert!(app
        .render_to_text(80, 12)
        .unwrap()
        .contains("audio unavailable"));
}

mod rows_by_reference {
    use super::*;
    use orbital_core::client::bridge::KernelBridge;
    use orbital_core::client::client_core::ClientCore;
    use orbital_core::client::client_kernel::ClientKernel;
    use orbital_core::runtime::row_query::{row_set_marker, RowSetRef};
    use serde_json::json;

    fn schema() -> OrbitalSchema {
        serde_json::from_value(json!({
            "name": "rows", "typeContractVersion": 1, "orbitals": [{
                "name": "Main",
                "entity": {
                    "name": "Ticket", "persistence": "runtime",
                    "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }, { "name": "owner", "type": "string" }],
                    "read_policy": ["!=", ["object/get", "@entity", "owner"], "u2"]
                },
                "traits": [{
                    "name": "Tickets", "linkedEntity": "Ticket",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "no list yet" }]] },
                            { "from": "idle", "to": "idle", "event": "LISTED", "effects": [["render-ui", "main", { "type": "table-view", "items": "@payload.data" }]] },
                            { "from": "idle", "to": "idle", "event": "RENAME", "effects": [["persist", "update", "Ticket", { "id": "t1", "title": "renamed" }]] },
                            { "from": "idle", "to": "idle", "event": "OTHER", "effects": [["render-ui", "main", { "type": "typography", "content": "plain" }]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Tickets" }] }]
            }]
        }))
        .unwrap()
    }

    fn row(id: &str, title: &str, owner: &str) -> orbital_core::Value {
        serde_json::from_value(json!({ "id": id, "title": title, "owner": owner })).unwrap()
    }

    fn app() -> (TuiApp, Arc<ClientKernel>) {
        let schema = schema();
        let server = KernelBridge::new(schema.clone()).unwrap();
        server
            .seed("Ticket", vec![row("t1", "alpha-row", "u1")])
            .unwrap();
        let bridge: Arc<dyn ServerBridge> = Arc::new(server);
        let kernel = Arc::new(ClientKernel::new(schema.clone(), Some(bridge)).unwrap());
        kernel
            .seed(
                "Ticket",
                vec![
                    row("t1", "alpha-row", "u1"),
                    row("t2", "hidden-row", "u2"),
                    row("t3", "gamma-row", "u1"),
                ],
            )
            .unwrap();
        let app = TuiApp::with_core(
            ClientCore::with_client_kernel(schema, kernel.clone()),
            Vec::new(),
        );
        (app, kernel)
    }

    fn list_all(app: &mut TuiApp) {
        let reference = RowSetRef {
            entity_type: "Ticket".into(),
            ids: vec!["t1".into(), "t2".into(), "t3".into()],
            total: 3,
            query: Default::default(),
        };
        let data = serde_json::to_value(row_set_marker(&reference)).unwrap();
        app.send_event(
            "LISTED",
            serde_json::from_value(json!({ "data": data, "totalCount": 3 })).unwrap(),
        );
    }

    #[test]
    fn a_marker_paints_the_rows_the_kernel_resolves_and_a_hidden_row_is_omitted() {
        let (mut app, _) = app();
        list_all(&mut app);
        let text = app.render_to_text(80, 12).unwrap();
        assert!(
            text.contains("alpha-row") && text.contains("gamma-row"),
            "{text}"
        );
        assert!(
            !text.contains("hidden-row"),
            "the policy hides u2's row: {text}"
        );
        assert!(!text.contains("$rowSet"), "{text}");
    }

    #[test]
    fn a_rows_changed_repaint_re_resolves_the_reference() {
        let (mut app, kernel) = app();
        list_all(&mut app);
        assert!(app.render_to_text(80, 12).unwrap().contains("alpha-row"));
        // the server runs the persist; the row it wrote reaches the client's rows before the repaint paints
        kernel
            .seed("Ticket", vec![row("t1", "renamed", "u1")])
            .unwrap();
        app.send_event("RENAME", orbital_core::Value::Null);
        let text = app.render_to_text(80, 12).unwrap();
        assert!(
            text.contains("renamed") && !text.contains("alpha-row"),
            "{text}"
        );
        assert!(text.contains("gamma-row"), "{text}");
    }

    #[test]
    fn control_a_slot_without_a_marker_paints_as_before() {
        let (mut app, _) = app();
        app.send_event("OTHER", orbital_core::Value::Null);
        assert!(app.render_to_text(80, 12).unwrap().contains("plain"));
    }
}

mod code_block_buffer {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use orbital_client_tui::components::core::molecules::code_block::{
        emit_key, emit_pointer, is_buffer_mode, paint_buffer, CodeBlockRenderer,
    };
    use orbital_client_tui::PatternRenderer;
    use orbital_core::client::helpers::{empty_trait_content, from_json, RenderContext};
    use orbital_core::Value;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;
    use std::collections::HashMap;

    fn props(json: serde_json::Value) -> Value {
        from_json(json)
    }

    fn painted(
        json: serde_json::Value,
        w: u16,
        h: u16,
    ) -> (Buffer, Option<ratatui::layout::Position>) {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        let caret = paint_buffer(&mut buf, area, &props(json));
        (buf, caret)
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn reversed(buf: &Buffer, x: u16, y: u16) -> bool {
        buf[(x, y)].modifier.contains(Modifier::REVERSED)
    }

    fn with_ctx<R>(
        f: impl FnOnce(&mut RenderContext<'_>) -> R,
    ) -> (R, Vec<(String, Option<String>, Value)>) {
        let entities = HashMap::new();
        let mut form = HashMap::new();
        let editing = None;
        let mut pending = Vec::new();
        let result = {
            let mut ctx = RenderContext {
                trait_name: "Ed",
                orbital_name: "Orb",
                current_state: "idle",
                entities: &entities,
                form_data: &mut form,
                editing_entity: &editing,
                pending_events: &mut pending,
                child_render: None,
                trait_content: empty_trait_content(),
                source_trait: None,
                canvas_theme: None,
            };
            f(&mut ctx)
        };
        (result, pending)
    }

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn buffer_mode_is_on_only_for_a_cursor_object() {
        assert!(is_buffer_mode(&props(
            serde_json::json!({ "cursor": { "row": 0, "col": 0 } })
        )));
        assert!(!is_buffer_mode(&props(serde_json::json!({ "code": "x" }))));
        assert!(!is_buffer_mode(&props(
            serde_json::json!({ "cursor": null })
        )));
    }

    #[test]
    fn control_without_a_cursor_the_viewer_draws_its_bordered_title() {
        let area = Rect::new(0, 0, 20, 4);
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| {
                let (_, _) = with_ctx(|ctx| {
                    CodeBlockRenderer.render(
                        frame,
                        area,
                        &props(serde_json::json!({ "code": "ab" })),
                        None,
                        ctx,
                    )
                });
            })
            .unwrap();
        let text: String = (0..20)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect();
        assert!(text.contains("code"), "{text}");
        let second: String = (0..20)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
            .collect();
        assert!(second.contains("ab"), "{second}");
    }

    #[test]
    fn the_renderer_in_buffer_mode_draws_no_border_and_sets_the_cursor_for_a_bar() {
        let area = Rect::new(0, 0, 20, 4);
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| {
                with_ctx(|ctx| {
                    CodeBlockRenderer.render(
                        frame,
                        area,
                        &props(serde_json::json!({ "code": "ab\ncde", "cursor": { "row": 1, "col": 2 }, "caret": "bar" })),
                        None,
                        ctx,
                    )
                });
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        assert_eq!(row_text(&buf, 0).trim_end(), "ab");
        assert_eq!(
            terminal.get_cursor_position().unwrap(),
            ratatui::layout::Position::new(2, 1)
        );
    }

    #[test]
    fn caret_block_covers_the_character_at_the_cursor() {
        let (buf, caret) = painted(
            serde_json::json!({ "code": "ab\ncde", "cursor": { "row": 1, "col": 2 } }),
            10,
            4,
        );
        assert!(reversed(&buf, 2, 1));
        assert_eq!(buf[(2, 1)].symbol(), "e");
        assert!(!reversed(&buf, 1, 1) && !reversed(&buf, 0, 1));
        assert_eq!(caret, Some(ratatui::layout::Position::new(2, 1)));
    }

    #[test]
    fn caret_bar_sits_on_the_cell_without_reversing_the_character() {
        let (buf, caret) = painted(
            serde_json::json!({ "code": "ab\ncde", "cursor": { "row": 1, "col": 2 }, "caret": "bar" }),
            10,
            4,
        );
        assert_eq!(caret, Some(ratatui::layout::Position::new(2, 1)));
        assert!(!reversed(&buf, 2, 1));
    }

    #[test]
    fn caret_underline_underlines_the_character() {
        let (buf, _) = painted(
            serde_json::json!({ "code": "ab\ncde", "cursor": { "row": 1, "col": 2 }, "caret": "underline" }),
            10,
            4,
        );
        assert!(buf[(2, 1)].modifier.contains(Modifier::UNDERLINED));
        assert!(!reversed(&buf, 2, 1));
        assert!(!buf[(1, 1)].modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn caret_at_line_end_is_a_space_width_block_after_the_last_character() {
        let (buf, _) = painted(
            serde_json::json!({ "code": "ab\ncde", "cursor": { "row": 0, "col": 2 } }),
            10,
            4,
        );
        assert_eq!(buf[(2, 0)].symbol(), " ");
        assert!(reversed(&buf, 2, 0));
        assert!(!reversed(&buf, 1, 0));
    }

    #[test]
    fn selection_linewise_paints_both_lines_and_not_the_third() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "ab\ncd\nef", "cursor": { "row": 2, "col": 0 },
                "selection": { "fromRow": 1, "fromCol": 1, "toRow": 0, "toCol": 0, "linewise": true }
            }),
            10,
            4,
        );
        for y in 0..2 {
            assert_eq!(buf[(0, y)].bg, Color::DarkGray);
            assert_eq!(buf[(1, y)].bg, Color::DarkGray);
        }
        assert_ne!(buf[(1, 2)].bg, Color::DarkGray);
    }

    #[test]
    fn selection_charwise_paints_bc_of_abcd_inclusive_of_the_end() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "abcd", "cursor": { "row": 0, "col": 3 },
                "selection": { "fromRow": 0, "fromCol": 1, "toRow": 0, "toCol": 2, "linewise": false }
            }),
            10,
            2,
        );
        let selected: Vec<bool> = (0..4).map(|x| buf[(x, 0)].bg == Color::DarkGray).collect();
        assert_eq!(selected, vec![false, true, true, false]);
    }

    #[test]
    fn selection_given_in_either_order_paints_the_same_range() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "abcd", "cursor": { "row": 0, "col": 3 },
                "selection": { "fromRow": 0, "fromCol": 2, "toRow": 0, "toCol": 1, "linewise": false }
            }),
            10,
            2,
        );
        let selected: Vec<bool> = (0..4).map(|x| buf[(x, 0)].bg == Color::DarkGray).collect();
        assert_eq!(selected, vec![false, true, true, false]);
    }

    #[test]
    fn decorations_keyword_colors_the_text_and_review_paints_a_background() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "fun x", "cursor": { "row": 0, "col": 4 },
                "decorations": [
                    { "id": "k", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 3, "tone": "keyword" },
                    { "id": "r", "fromRow": 0, "fromCol": 4, "toRow": 0, "toCol": 5, "tone": "review" },
                    { "id": "u", "fromRow": 0, "fromCol": 3, "toRow": 0, "toCol": 4, "tone": "no-such-tone" }
                ]
            }),
            10,
            2,
        );
        for x in 0..3 {
            assert_eq!(buf[(x, 0)].fg, Color::Magenta);
            assert_eq!(buf[(x, 0)].bg, Color::Reset);
        }
        assert_eq!(buf[(3, 0)].fg, Color::Reset);
        assert_eq!(buf[(3, 0)].bg, Color::Reset);
        assert_eq!(buf[(4, 0)].bg, Color::Blue);
    }

    #[test]
    fn later_decorations_paint_over_earlier_ones() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "ab", "cursor": { "row": 0, "col": 1 },
                "decorations": [
                    { "id": "a", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 1, "tone": "string" },
                    { "id": "b", "fromRow": 0, "fromCol": 0, "toRow": 0, "toCol": 1, "tone": "number" }
                ]
            }),
            10,
            2,
        );
        assert_eq!(buf[(0, 0)].fg, Color::Yellow);
    }

    #[test]
    fn popover_renders_title_heading_lines_and_the_busy_indicator_below_the_range() {
        let json = |busy: bool| {
            serde_json::json!({
                "code": "one\ntwo\nthree", "cursor": { "row": 0, "col": 0 },
                "popover": {
                    "fromRow": 0, "fromCol": 1, "toRow": 0, "toCol": 3, "title": "Note",
                    "sections": [{ "heading": "Why", "lines": ["because", "reasons"] }], "busy": busy
                }
            })
        };
        let (buf, _) = painted(json(true), 30, 12);
        let rows: Vec<String> = (0..12).map(|y| row_text(&buf, y)).collect();
        let at = |needle: &str| {
            rows.iter()
                .position(|r| r.contains(needle))
                .unwrap_or_else(|| panic!("{needle}: {rows:?}"))
        };
        assert!(at("Note") > 0);
        assert!(at("Why") > at("Note"));
        assert!(at("because") > at("Why"));
        assert!(at("reasons") > at("because"));
        assert!(at("…") > at("reasons"));
        let (control, _) = painted(json(false), 30, 12);
        assert!(!(0..12).any(|y| row_text(&control, y).contains('…')));
    }

    #[test]
    fn popover_flips_above_when_there_is_no_room_below_and_null_renders_nothing() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "a\nb\nc\nd", "cursor": { "row": 3, "col": 0 },
                "popover": { "fromRow": 3, "fromCol": 0, "toRow": 3, "toCol": 1, "title": "Tip", "sections": [], "busy": false }
            }),
            20,
            4,
        );
        let rows: Vec<String> = (0..4).map(|y| row_text(&buf, y)).collect();
        let tip = rows.iter().position(|r| r.contains("Tip")).unwrap();
        assert!(tip < 3, "{rows:?}");
        let (none, _) = painted(
            serde_json::json!({ "code": "a", "cursor": { "row": 0, "col": 0 }, "popover": null }),
            20,
            4,
        );
        assert!(!(0..4).any(|y| row_text(&none, y).contains('┌')));
    }

    #[test]
    fn relative_line_numbers_show_distance_and_the_cursor_row_its_absolute_number() {
        let (buf, _) = painted(
            serde_json::json!({
                "code": "a\nb\nc\nd\ne", "cursor": { "row": 2, "col": 0 },
                "lineNumbers": true, "relativeLineNumbers": true
            }),
            10,
            6,
        );
        let gutter: Vec<String> = (0..5).map(|y| buf[(0, y)].symbol().to_string()).collect();
        assert_eq!(gutter, vec!["2", "1", "3", "1", "2"]);
        let (absolute, _) = painted(
            serde_json::json!({ "code": "a\nb\nc", "cursor": { "row": 1, "col": 0 }, "lineNumbers": true }),
            10,
            4,
        );
        let gutter: Vec<String> = (0..3)
            .map(|y| absolute[(0, y)].symbol().to_string())
            .collect();
        assert_eq!(gutter, vec!["1", "2", "3"]);
        assert_eq!(absolute[(2, 0)].symbol(), "a");
    }

    #[test]
    fn the_view_scrolls_to_keep_the_cursor_line_visible() {
        let code: Vec<String> = (0..10).map(|i| format!("l{i}")).collect();
        let (buf, caret) = painted(
            serde_json::json!({ "code": code.join("\n"), "cursor": { "row": 8, "col": 0 } }),
            10,
            3,
        );
        assert_eq!(row_text(&buf, 2).trim_end(), "l8");
        assert_eq!(caret, Some(ratatui::layout::Position::new(0, 2)));
        let (top, _) = painted(
            serde_json::json!({ "code": code.join("\n"), "cursor": { "row": 1, "col": 0 } }),
            10,
            3,
        );
        assert_eq!(row_text(&top, 0).trim_end(), "l0");
    }

    #[test]
    fn a_key_press_queues_onkey_with_the_web_payload() {
        let p = props(
            serde_json::json!({ "onKey": "KEY", "editorId": "ed1", "cursor": { "row": 0, "col": 0 } }),
        );
        let (sent, pending) = with_ctx(|ctx| {
            let a = emit_key(&p, ctx, &press(KeyCode::Esc, KeyModifiers::NONE));
            let b = emit_key(&p, ctx, &press(KeyCode::Char('r'), KeyModifiers::CONTROL));
            let c = emit_key(&p, ctx, &press(KeyCode::Left, KeyModifiers::SHIFT));
            (a, b, c)
        });
        assert_eq!(sent, (true, true, true));
        let payload = |i: usize| orbital_core::client::helpers::to_json(&pending[i].2);
        assert_eq!(pending[0].0, "KEY");
        assert_eq!(
            payload(0),
            serde_json::json!({ "editorId": "ed1", "key": "Escape", "code": "Escape", "ctrl": false, "alt": false, "shift": false, "meta": false, "repeat": false })
        );
        assert_eq!(payload(1)["key"], "r");
        assert_eq!(payload(1)["code"], "KeyR");
        assert_eq!(payload(1)["ctrl"], true);
        assert_eq!(payload(2)["key"], "ArrowLeft");
        assert_eq!(payload(2)["shift"], true);
    }

    #[test]
    fn control_a_release_or_a_block_without_onkey_emits_nothing() {
        let with = props(serde_json::json!({ "onKey": "KEY", "cursor": { "row": 0, "col": 0 } }));
        let without = props(serde_json::json!({ "cursor": { "row": 0, "col": 0 } }));
        let mut release = press(KeyCode::Char('a'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        let ((a, b), pending) = with_ctx(|ctx| {
            (
                emit_key(&with, ctx, &release),
                emit_key(
                    &without,
                    ctx,
                    &press(KeyCode::Char('a'), KeyModifiers::NONE),
                ),
            )
        });
        assert!(!a && !b && pending.is_empty());
    }

    #[test]
    fn a_pointer_press_queues_row_and_col_with_clamping() {
        let p = props(serde_json::json!({
            "onPointerDown": "PTR", "onHover": "HOV", "editorId": "ed1", "code": "ab\ncde",
            "cursor": { "row": 0, "col": 0 }, "lineNumbers": true
        }));
        let area = Rect::new(0, 0, 10, 4);
        let (_, pending) = with_ctx(|ctx| {
            emit_pointer(&p, ctx, area, "onPointerDown", 3, 1);
            emit_pointer(&p, ctx, area, "onPointerDown", 9, 0);
            emit_pointer(&p, ctx, area, "onPointerDown", 9, 3);
            emit_pointer(&p, ctx, area, "onHover", 2, 0);
            emit_pointer(&p, ctx, area, "onPointerDown", 30, 0)
        });
        let sent: Vec<(String, serde_json::Value)> = pending
            .iter()
            .map(|(e, _, v)| (e.clone(), orbital_core::client::helpers::to_json(v)))
            .collect();
        assert_eq!(pending.len(), 4);
        assert_eq!(
            sent[0],
            (
                "PTR".into(),
                serde_json::json!({ "editorId": "ed1", "row": 1, "col": 1 })
            )
        );
        assert_eq!(sent[1].1["col"], 2);
        assert_eq!(
            sent[2].1,
            serde_json::json!({ "editorId": "ed1", "row": 1, "col": 3 })
        );
        assert_eq!(sent[3].0, "HOV");
    }
}
