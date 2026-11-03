//! The renderer mounts a program `orb` already resolved; it never resolves one.

use orbital_client_dioxus::shell::{mount_core, snapshot_json};

fn program(traits: serde_json::Value) -> String {
    serde_json::json!({
        "name": "counter",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Counter", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": traits,
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Show" }] }]
        }]
    })
    .to_string()
}

#[test]
fn a_resolved_program_mounts_and_renders_its_main_slot() {
    let resolved = program(serde_json::json!([{
        "name": "Show", "linkedEntity": "Counter",
        "stateMachine": {
            "states": [{ "name": "idle", "isInitial": true }],
            "transitions": [{ "from": "idle", "to": "idle", "event": "INIT",
                "effects": [["render-ui", "main", { "type": "typography", "content": "hello" }]] }]
        }
    }]));
    let (orbitals, traits) = mount_core(&resolved).expect("mount");
    assert_eq!((orbitals, traits), (1, 1));
    assert!(snapshot_json().unwrap().contains("hello"));
}

#[test]
fn control_malformed_json_is_refused_not_resolved() {
    assert!(mount_core("{ not json").is_err());
}

#[test]
fn persistent_entity_without_instances_does_not_get_a_synthetic_row() {
    let resolved = serde_json::json!({
        "name": "todos",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Todo", "persistence": "persistent", "fields": [
                { "name": "id", "type": "string", "required": true },
                { "name": "title", "type": "string", "required": true }
            ] },
            "traits": [{ "name": "Show", "linkedEntity": "Todo", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT",
                    "effects": [["render-ui", "main", { "type": "typography", "content": "Tasks" }]] }]
            } }],
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Show" }] }]
        }]
    }).to_string();
    mount_core(&resolved).expect("persistent row is created only by an authored effect");
    assert!(snapshot_json().unwrap().contains("Tasks"));
}

#[cfg(feature = "native")]
fn skeleton_tree_app() -> dioxus::prelude::Element {
    use orbital_client_dioxus::{registry::default_registry, DioxusView};
    let spec: orbital_core::Value = serde_json::from_str(
        r#"{"type":"skeleton","spec":{"stack":[{"shape":{"variant":"table","rows":2,"columns":3}},{"shape":{"variant":"text","rows":1}}]}}"#,
    )
    .unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let view = DioxusView {
        trait_name: "Load",
        orbital_name: "Main",
        current_state: "loading",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: None,
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
    };
    default_registry().render_spec(&spec, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn skeleton_tree_paints_the_authored_stack_shape() {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};

    let mut dom = VirtualDom::new(skeleton_tree_app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    assert!(initial.edits.iter().any(|edit| matches!(edit,
        Mutation::SetAttribute { name: "data-skeleton", value: AttributeValue::Text(value), .. } if value == "tree"
    )));
}

#[cfg(feature = "native")]
static CONTROL_PATTERN: std::sync::Mutex<&str> = std::sync::Mutex::new("");
#[cfg(feature = "native")]
static CONTROL_EVENTS: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn control_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::DioxusView;

    let props: orbital_core::Value =
        serde_json::from_str(*CONTROL_PATTERN.lock().unwrap()).unwrap();
    let registry = orbital_client_dioxus::registry::default_registry();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        CONTROL_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_string(&payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "TodoView",
        orbital_name: "NativeTodoOrbital",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
static FLOW_EVENTS: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn flow_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::DioxusView;

    let props: orbital_core::Value = serde_json::from_str(r#"{
      "type":"flow-canvas", "initialLevel":"trait-expanded", "initialOrbital":"Main",
      "traitView":{"lens":"flow","filter":null}, "flowSelectEvent":"SELECT_FLOW",
      "schema":{"name":"app","typeContractVersion":1,"orbitals":[{"name":"Main",
        "entity":{"name":"Item","fields":[]},"traits":[
          {"name":"One","sourceBehavior":{"behavior":"std/atoms/flow","alias":"Flow","originalName":"One"},
           "stateMachine":{"states":[{"name":"idle"}],"transitions":[]}},
          {"name":"Two","sourceBehavior":{"behavior":"std/atoms/flow","alias":"Flow","originalName":"Two"},
           "stateMachine":{"states":[{"name":"idle"}],"transitions":[]}}
        ]}]}}
    "#).unwrap();
    let registry = orbital_client_dioxus::registry::default_registry();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        FLOW_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_string(&payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "One",
        orbital_name: "Main",
        current_state: "idle",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn flow_canvas_selects_a_core_grouped_unit() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    FLOW_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(flow_app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    let button = initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .expect("flow group click listener");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    dom.runtime()
        .handle_event("click", Event::new(mouse, true), button);
    let mut selected = Mutations::default();
    dom.render_immediate(&mut selected);
    let events = FLOW_EVENTS.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "SELECT_FLOW");
    let payload: serde_json::Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(payload["id"], "behavior:flow");
    assert_eq!(payload["traits"], serde_json::json!(["One", "Two"]));
}

#[cfg(feature = "native")]
#[test]
fn atomic_controls_emit_the_declared_event_payloads() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    let dispatch = |pattern: &'static str, event_name: &str, data: Rc<dyn Any>| {
        *CONTROL_PATTERN.lock().unwrap() = pattern;
        CONTROL_EVENTS.lock().unwrap().clear();
        let mut dom = VirtualDom::new(control_app);
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        let id = mutations
            .edits
            .iter()
            .find_map(|edit| match edit {
                Mutation::NewEventListener { name, id } if name == event_name => Some(*id),
                _ => None,
            })
            .expect("control event listener");
        dom.runtime()
            .handle_event(event_name, Event::new(data, true), id);
        CONTROL_EVENTS.lock().unwrap().clone()
    };
    let mouse = || {
        Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>
    };
    assert_eq!(
        dispatch(
            r#"{"type":"button","label":"Delete","action":"DELETE","actionPayload":{"id":"task-7"}}"#,
            "click",
            mouse()
        ),
        vec![("DELETE".to_string(), r#"{"id":"task-7"}"#.to_string())]
    );
    assert!(dispatch(r#"{"type":"button","label":"Delete","action":"DELETE","disabled":true,"actionPayload":{"id":"task-7"}}"#, "click", mouse()).is_empty());
    let form = Rc::new(PlatformEventData::new(Box::new(
        dioxus_html::SerializedFormData::new("Buy milk".to_string(), vec![]),
    ))) as Rc<dyn Any>;
    assert_eq!(
        dispatch(
            r#"{"type":"input","label":"New task","action":"DRAFT_CHANGED"}"#,
            "input",
            form
        ),
        vec![(
            "DRAFT_CHANGED".to_string(),
            r#"{"value":"Buy milk"}"#.to_string()
        )]
    );
    assert_eq!(
        dispatch(
            r#"{"type":"checkbox","label":"Complete","checked":false,"onChange":"SET_DONE"}"#,
            "click",
            mouse()
        ),
        vec![("SET_DONE".to_string(), r#"{"checked":true}"#.to_string())]
    );
    assert_eq!(
        dispatch(
            r#"{"type":"checkbox","label":"Complete","checked":true,"onChange":"SET_DONE"}"#,
            "click",
            mouse()
        ),
        vec![("SET_DONE".to_string(), r#"{"checked":false}"#.to_string())]
    );
    let disabled_form = Rc::new(PlatformEventData::new(Box::new(
        dioxus_html::SerializedFormData::new("Ignored".to_string(), vec![]),
    ))) as Rc<dyn Any>;
    assert!(dispatch(
        r#"{"type":"input","label":"New task","action":"DRAFT_CHANGED","disabled":true}"#,
        "input",
        disabled_form
    )
    .is_empty());
    let number_form = Rc::new(PlatformEventData::new(Box::new(
        dioxus_html::SerializedFormData::new("42".to_string(), vec![]),
    ))) as Rc<dyn Any>;
    assert_eq!(
        dispatch(
            r#"{"type":"input","label":"Estimate","inputType":"number","action":"ESTIMATE_CHANGED"}"#,
            "input",
            number_form
        ),
        vec![(
            "ESTIMATE_CHANGED".to_string(),
            r#"{"value":42.0}"#.to_string()
        )]
    );
}

#[cfg(feature = "native")]
static NEW_CONTROL_PATTERN: std::sync::Mutex<&str> = std::sync::Mutex::new("");
#[cfg(feature = "native")]
static NEW_CONTROL_EVENTS: std::sync::Mutex<Vec<(String, String)>> =
    std::sync::Mutex::new(Vec::new());
#[cfg(feature = "native")]
static NEW_CONTROL_NAVIGATIONS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
#[cfg(feature = "native")]
static NEW_CONTROL_VIEWPORT_WIDTH: std::sync::Mutex<Option<f64>> = std::sync::Mutex::new(None);
#[cfg(feature = "native")]
static NEW_CONTROL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "native")]
fn new_control_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::components::{core, game};
    use orbital_client_dioxus::{registry::DioxusPatternRegistry, DioxusView};

    let props: orbital_core::Value =
        serde_json::from_str(*NEW_CONTROL_PATTERN.lock().unwrap()).unwrap();
    let mut registry = DioxusPatternRegistry::new();
    registry.register(Box::new(core::organisms::dock_layout::DockLayoutRenderer));
    registry.register(Box::new(core::atoms::typography::TypographyRenderer));
    registry.register(Box::new(
        core::molecules::activity_rail::ActivityRailRenderer,
    ));
    registry.register(Box::new(game::atoms::choice_button::ChoiceButtonRenderer));
    registry.register(Box::new(game::atoms::control_button::ControlButtonRenderer));
    registry.register(Box::new(
        game::atoms::game_audio_toggle::GameAudioToggleRenderer,
    ));
    registry.register(Box::new(game::molecules::control_grid::ControlGridRenderer));
    registry.register(Box::new(game::molecules::game_menu::GameMenuRenderer));
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        NEW_CONTROL_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_string(&payload).unwrap()));
    });
    let navigate = Callback::new(|path: String| {
        NEW_CONTROL_NAVIGATIONS.lock().unwrap().push(path);
    });
    let viewport_width = *NEW_CONTROL_VIEWPORT_WIDTH.lock().unwrap();
    let view = DioxusView {
        trait_name: "ControlView",
        orbital_name: "ControlOrbital",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        navigate: Some(navigate),
        dock: None,
        viewport_width,
        canvas_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
fn new_control_events(pattern: &'static str, names: &[&str]) -> Vec<(String, String)> {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    *NEW_CONTROL_PATTERN.lock().unwrap() = pattern;
    NEW_CONTROL_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(new_control_app);
    let mut mutations = Mutations::default();
    dom.rebuild(&mut mutations);
    let listeners: Vec<(String, dioxus_core::ElementId)> = mutations
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } => Some((name.clone(), *id)),
            _ => None,
        })
        .collect();
    for name in names {
        let id = listeners
            .iter()
            .find_map(|(registered, id)| (registered.as_str() == *name).then_some(*id))
            .expect("control event listener");
        let mouse = Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>;
        dom.runtime()
            .handle_event(name, Event::new(mouse, *name != "mouseleave"), id);
    }
    NEW_CONTROL_EVENTS.lock().unwrap().clone()
}

#[cfg(feature = "native")]
#[test]
fn compact_activity_rail_opens_its_authored_panel_after_selection() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    *NEW_CONTROL_PATTERN.lock().unwrap() = r#"{"type":"dock-layout","compact":true,"rail":{"type":"activity-rail","items":[{"id":"assets","label":"Assets","icon":"folder","opensPanel":true}],"onChange":"PICK","onSelect":"CALLBACK"},"sidebar":{"type":"typography","content":"Asset panel"},"main":{"type":"typography","content":"Main"}}"#;
    NEW_CONTROL_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(new_control_app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    let menu = initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .expect("compact menu button");
    let mouse = || {
        Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>
    };
    dom.runtime()
        .handle_event("click", Event::new(mouse(), true), menu);
    let mut opened = Mutations::default();
    dom.render_immediate(&mut opened);
    let rail_item = opened
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .last()
        .expect("rail item in compact drawer");
    dom.runtime()
        .handle_event("click", Event::new(mouse(), true), rail_item);
    let mut selected = Mutations::default();
    dom.render_immediate(&mut selected);
    assert_eq!(
        *NEW_CONTROL_EVENTS.lock().unwrap(),
        vec![
            ("PICK".into(), r#"{"id":"assets"}"#.into()),
            ("CALLBACK".into(), r#"{"id":"assets"}"#.into())
        ]
    );
    assert!(selected.edits.iter().any(
        |edit| matches!(edit, Mutation::CreateTextNode { value, .. } if value == "Asset panel")
    ));
}

#[cfg(feature = "native")]
#[test]
fn compact_activity_rail_closes_menu_for_an_item_without_a_panel() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    *NEW_CONTROL_PATTERN.lock().unwrap() = r#"{"type":"dock-layout","compact":true,"rail":{"type":"activity-rail","items":[{"id":"dialog","label":"Dialog","icon":"circle","opensPanel":false}],"onChange":"PICK"},"sidebar":{"type":"typography","content":"Panel must stay closed"},"main":{"type":"typography","content":"Main"}}"#;
    NEW_CONTROL_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(new_control_app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    let menu = initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .expect("compact menu button");
    let mouse = || {
        Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>
    };
    dom.runtime()
        .handle_event("click", Event::new(mouse(), true), menu);
    let mut opened = Mutations::default();
    dom.render_immediate(&mut opened);
    let rail_item = opened
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .last()
        .expect("rail item in compact drawer");
    dom.runtime()
        .handle_event("click", Event::new(mouse(), true), rail_item);
    let mut closed = Mutations::default();
    dom.render_immediate(&mut closed);
    assert_eq!(
        *NEW_CONTROL_EVENTS.lock().unwrap(),
        vec![("PICK".into(), r#"{"id":"dialog"}"#.into())]
    );
    assert!(closed
        .edits
        .iter()
        .any(|edit| matches!(edit, Mutation::Remove { .. } | Mutation::ReplaceWith { .. })));
    assert!(!closed.edits.iter().any(|edit| matches!(edit, Mutation::CreateTextNode { value, .. } if value == "Panel must stay closed")));
}

#[cfg(feature = "native")]
#[test]
fn compact_dock_drawer_closes_when_viewport_becomes_desktop_width() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations, ScopeId};
    use std::{any::Any, rc::Rc};

    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    *NEW_CONTROL_VIEWPORT_WIDTH.lock().unwrap() = Some(800.0);
    *NEW_CONTROL_PATTERN.lock().unwrap() = r#"{"type":"dock-layout","rail":{"type":"activity-rail","items":[{"id":"assets","label":"Assets","icon":"folder"}],"onChange":"PICK"},"main":{"type":"typography","content":"Main"}}"#;
    NEW_CONTROL_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(new_control_app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    let menu = initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .expect("compact menu button");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    dom.runtime()
        .handle_event("click", Event::new(mouse, true), menu);
    let mut opened = Mutations::default();
    dom.render_immediate(&mut opened);
    assert!(opened
        .edits
        .iter()
        .any(|edit| matches!(edit, Mutation::NewEventListener { name, .. } if name == "click")));

    *NEW_CONTROL_VIEWPORT_WIDTH.lock().unwrap() = Some(1200.0);
    dom.mark_dirty(ScopeId::APP);
    let mut desktop = Mutations::default();
    dom.render_immediate(&mut desktop);
    let mut settled = Mutations::default();
    dom.render_immediate(&mut settled);
    let desktop_item = desktop
        .edits
        .iter()
        .chain(settled.edits.iter())
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .last()
        .expect("desktop rail item is painted after resize");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    dom.runtime()
        .handle_event("click", Event::new(mouse, true), desktop_item);
    assert_eq!(
        *NEW_CONTROL_EVENTS.lock().unwrap(),
        vec![("PICK".into(), r#"{"id":"assets"}"#.into())]
    );
    *NEW_CONTROL_VIEWPORT_WIDTH.lock().unwrap() = None;
}

#[cfg(feature = "native")]
#[test]
fn new_atomic_controls_emit_only_declared_events() {
    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    assert_eq!(
        new_control_events(
            r#"{"type":"choice-button","text":"Open","action":"CHOOSE","payload":{"nextId":"scene-2"},"onClick":"CALLBACK"}"#,
            &["click"]
        ),
        vec![
            ("CHOOSE".into(), r#"{"nextId":"scene-2"}"#.into()),
            ("CALLBACK".into(), "null".into()),
        ]
    );
    assert_eq!(
        new_control_events(
            r#"{"type":"activity-rail","items":[{"id":"assets","label":"Assets","icon":"folder"}],"onChange":"PICK","onSelect":"CALLBACK"}"#,
            &["click"]
        ),
        vec![
            ("PICK".into(), r#"{"id":"assets"}"#.into()),
            ("CALLBACK".into(), r#"{"id":"assets"}"#.into()),
        ]
    );
    let button = r#"{"type":"control-button","label":"Fire","pressEvent":"FIRE","releaseEvent":"STOP","onPress":"CALLBACK","onRelease":"CALLBACK"}"#;
    assert_eq!(
        new_control_events(button, &["mousedown", "mouseup", "click"]),
        vec![
            ("FIRE".into(), "null".into()),
            ("CALLBACK".into(), "null".into()),
            ("STOP".into(), "null".into()),
            ("CALLBACK".into(), "null".into()),
        ]
    );
    assert_eq!(
        new_control_events(button, &["mousedown", "mouseleave", "click"]),
        vec![
            ("FIRE".into(), "null".into()),
            ("CALLBACK".into(), "null".into()),
            ("STOP".into(), "null".into()),
            ("CALLBACK".into(), "null".into()),
        ]
    );
}

#[cfg(feature = "native")]
#[test]
fn game_audio_toggle_emits_authored_next_state_only() {
    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    assert_eq!(
        new_control_events(
            r#"{"type":"game-audio-toggle","muted":false,"soundOnLabel":"Sound on","toggleEvent":"TOGGLE"}"#,
            &["click"]
        ),
        vec![("TOGGLE".into(), r#"{"muted":true}"#.into())]
    );
    assert_eq!(
        new_control_events(
            r#"{"type":"game-audio-toggle","muted":true,"mutedLabel":"Muted","toggleEvent":"TOGGLE"}"#,
            &["click"]
        ),
        vec![("TOGGLE".into(), r#"{"muted":false}"#.into())]
    );
    assert!(new_control_events(
        r#"{"type":"game-audio-toggle","muted":false,"isLoading":true,"toggleEvent":"TOGGLE"}"#,
        &["click"]
    )
    .is_empty());
}

#[cfg(feature = "native")]
#[test]
fn control_grid_reuses_button_press_and_release_with_typed_payloads() {
    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    assert_eq!(
        new_control_events(
            r#"{"type":"control-grid","kind":"actions","visibility":"always","buttons":[{"id":"jump","label":"Jump"}],"actionEvent":"ACTION","onAction":"CALLBACK"}"#,
            &["mousedown", "mouseup", "click"]
        ),
        vec![
            ("ACTION".into(), r#"{"id":"jump","pressed":true}"#.into()),
            ("CALLBACK".into(), r#"{"id":"jump","pressed":true}"#.into()),
            ("ACTION".into(), r#"{"id":"jump","pressed":false}"#.into()),
            ("CALLBACK".into(), r#"{"id":"jump","pressed":false}"#.into()),
        ]
    );
    assert_eq!(
        new_control_events(
            r#"{"type":"control-grid","kind":"dpad","visibility":"always","directionEvents":{"up":"MOVE"},"directionReleaseEvents":{"up":"STOP"},"actionEvent":"ACTION","directionEvent":"DIRECTION","onDirection":"CALLBACK"}"#,
            &["mousedown", "mouseup", "click"]
        ),
        vec![
            ("ACTION".into(), r#"{"id":"up","pressed":true}"#.into()),
            ("MOVE".into(), "null".into()),
            (
                "DIRECTION".into(),
                r#"{"direction":"up","pressed":true}"#.into()
            ),
            (
                "CALLBACK".into(),
                r#"{"direction":"up","pressed":true}"#.into()
            ),
            ("ACTION".into(), r#"{"id":"up","pressed":false}"#.into()),
            ("STOP".into(), "null".into()),
            (
                "DIRECTION".into(),
                r#"{"direction":"up","pressed":false}"#.into()
            ),
            (
                "CALLBACK".into(),
                r#"{"direction":"up","pressed":false}"#.into()
            ),
        ]
    );
}

#[cfg(feature = "native")]
#[test]
fn game_menu_dispatches_option_event_callback_and_kernel_navigation() {
    let _guard = NEW_CONTROL_TEST_LOCK.lock().unwrap();
    NEW_CONTROL_NAVIGATIONS.lock().unwrap().clear();
    let option = serde_json::json!({
        "id": "play",
        "label": "Play",
        "event": "START",
        "navigatesTo": "/play"
    });
    let events = new_control_events(
        r#"{"type":"game-menu","title":"Menu","options":[{"id":"play","label":"Play","event":"START","navigatesTo":"/play"}],"onSelect":"CHOSEN"}"#,
        &["click"],
    );
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "START");
    assert_eq!(events[1].0, "CHOSEN");
    for (_, payload) in events {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&payload).unwrap(),
            serde_json::json!({"option": option})
        );
    }
    assert_eq!(
        NEW_CONTROL_NAVIGATIONS.lock().unwrap().as_slice(),
        &["/play"]
    );

    NEW_CONTROL_NAVIGATIONS.lock().unwrap().clear();
    assert!(new_control_events(
        r#"{"type":"game-menu","title":"Menu","options":[{"label":"Disabled","event":"START","navigatesTo":"/play","disabled":true}],"onSelect":"CHOSEN"}"#,
        &["click"],
    ).is_empty());
    assert!(NEW_CONTROL_NAVIGATIONS.lock().unwrap().is_empty());

    assert_eq!(
        new_control_events(
            r#"{"type":"game-menu","title":"Menu","options":[],"menuItems":[{"label":"Continue","event":"RESUME"}]}"#,
            &["click"],
        )[0].0,
        "RESUME",
    );
}

#[cfg(all(feature = "native", feature = "research-drag"))]
static DRAG_SLOT_PATTERN: std::sync::Mutex<&str> = std::sync::Mutex::new("");

#[cfg(all(feature = "native", feature = "research-drag"))]
fn drag_slot_app() -> dioxus::prelude::Element {
    use orbital_client_dioxus::DioxusView;

    let props: orbital_core::Value =
        serde_json::from_str(*DRAG_SLOT_PATTERN.lock().unwrap()).unwrap();
    let registry = orbital_client_dioxus::registry::default_registry();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let view = DioxusView {
        trait_name: "SlotView",
        orbital_name: "SlotOrbital",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: None,
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(all(feature = "native", feature = "research-drag"))]
#[test]
fn trait_slot_empty_remove_event_does_not_add_a_remove_control() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};

    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    let count_clicks = |pattern: &'static str| {
        *DRAG_SLOT_PATTERN.lock().unwrap() = pattern;
        let mut dom = VirtualDom::new(drag_slot_app);
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        mutations
            .edits
            .iter()
            .filter(
                |edit| matches!(edit, Mutation::NewEventListener { name, .. } if name == "click"),
            )
            .count()
    };
    let slot = r#"{"type":"trait-slot","slotNumber":1,"equippedItem":{"id":"jump","name":"Jump","category":"move"},"removeEvent":""}"#;
    let removable = r#"{"type":"trait-slot","slotNumber":1,"equippedItem":{"id":"jump","name":"Jump","category":"move"},"removeEvent":"REMOVE"}"#;
    assert_eq!(count_clicks(slot), 1);
    assert_eq!(count_clicks(removable), 2);
}
