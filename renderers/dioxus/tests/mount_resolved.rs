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
        source_sink: dioxus::prelude::Callback::new(|_: Option<String>| {
            dioxus::prelude::Callback::new(|_: (String, orbital_core::Value)| {})
        }),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
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
static GRID_SELECTION_EVENTS: std::sync::Mutex<Vec<(String, serde_json::Value)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn selectable_grid_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};

    let props: orbital_core::Value = serde_json::from_value(serde_json::json!({
        "type": "data-grid",
        "rows": [
            {"id": "todo-1", "title": "Alpha"},
            {"id": "todo-2", "title": "Beta"}
        ],
        "fields": [{"name": "title", "variant": "h3"}],
        "selectable": true,
        "selectionEvent": "SELECT_TODOS"
    }))
    .unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        GRID_SELECTION_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_value(payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "TodoView",
        orbital_name: "Main",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    default_registry().render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn data_grid_selection_toggles_locally_and_emits_only_declared_payload() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use dioxus_core::ElementId;
    use dioxus_native::{DioxusDocument, DocumentConfig};
    use std::{any::Any, rc::Rc};

    GRID_SELECTION_EVENTS.lock().unwrap().clear();
    let mut doc = DioxusDocument::new(
        VirtualDom::new(selectable_grid_app),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc.poll(None);
    assert_eq!(
        doc.query_selector_all("[data-entity-row]").unwrap().len(),
        2
    );
    let checkbox = doc
        .query_selector("button[role='checkbox'][aria-label='Select Alpha']")
        .unwrap()
        .expect("first row checkbox");
    let dioxus_id = doc
        .get_node(checkbox)
        .and_then(|node| node.element_data())
        .and_then(|element| {
            element
                .attrs
                .iter()
                .find(|attr| *attr.name.local == *"data-dioxus-id")
        })
        .and_then(|attr| attr.value.parse::<usize>().ok())
        .map(ElementId)
        .expect("checkbox Dioxus event target");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    doc.vdom
        .runtime()
        .handle_event("click", Event::new(mouse, true), dioxus_id);
    doc.poll(None);

    assert_eq!(
        *GRID_SELECTION_EVENTS.lock().unwrap(),
        vec![(
            "SELECT_TODOS".to_string(),
            serde_json::json!({"selectedIds": ["todo-1"]})
        )]
    );
    assert!(doc
        .query_selector("[data-entity-id='todo-1'][aria-current='true']")
        .unwrap()
        .is_some());
    assert_eq!(
        doc.query_selector_all("[data-entity-row]").unwrap().len(),
        2
    );
    assert!(doc
        .query_selector("[data-entity-id='todo-2']")
        .unwrap()
        .is_some());
}

#[cfg(feature = "native")]
static ROW_ROUTE_SPEC: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
#[cfg(feature = "native")]
static ROW_ROUTE_NAVIGATIONS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
#[cfg(feature = "native")]
static ROW_ROUTE_EVENTS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn row_route_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};

    let props: orbital_core::Value = serde_json::from_str(&ROW_ROUTE_SPEC.lock().unwrap()).unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, _): (String, orbital_core::Value)| {
        ROW_ROUTE_EVENTS.lock().unwrap().push(event);
    });
    let navigate = Callback::new(|route: String| {
        ROW_ROUTE_NAVIGATIONS.lock().unwrap().push(route);
    });
    let view = DioxusView {
        trait_name: "RowRoutes",
        orbital_name: "Main",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: Some(navigate),
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    default_registry().render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn row_route_actions_navigate_without_emitting_for_grid_and_list() {
    use blitz_dom::{Document as _, DocumentConfig};
    use dioxus::prelude::*;
    use dioxus_core::ElementId;
    use dioxus_native::DioxusDocument;
    use std::{any::Any, rc::Rc};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for (pattern, destination, expected) in [
        ("data-grid", serde_json::json!("/tasks"), "/tasks"),
        (
            "data-grid",
            serde_json::json!(["fn", "row", ["str/concat", "/tasks/", "@row.id"]]),
            "/tasks/todo-1",
        ),
        (
            "data-list",
            serde_json::json!(["fn", "row", ["str/concat", "/tasks/", "@row.id"]]),
            "/tasks/todo-1",
        ),
    ] {
        *ROW_ROUTE_SPEC.lock().unwrap() = serde_json::json!({
            "type": pattern,
            "rows": [{"id": "todo-1", "title": "Milk"}],
            "fields": [{"name": "title", "variant": "h3"}],
            "itemActions": [{
                "label": "Open", "variant": "primary", "navigatesTo": destination,
                "event": "SHOULD_NOT_EMIT"
            }]
        })
        .to_string();
        ROW_ROUTE_NAVIGATIONS.lock().unwrap().clear();
        ROW_ROUTE_EVENTS.lock().unwrap().clear();
        let mut doc =
            DioxusDocument::new(VirtualDom::new(row_route_app), DocumentConfig::default());
        doc.initial_build();
        doc.poll(None);
        let anchor = doc
            .query_selector(&format!("a[href='{expected}']"))
            .unwrap()
            .expect("resolved row route anchor");
        let id = doc
            .get_node(anchor)
            .and_then(|node| node.element_data())
            .and_then(|element| {
                element
                    .attrs
                    .iter()
                    .find(|attr| *attr.name.local == *"data-dioxus-id")
            })
            .and_then(|attr| attr.value.parse::<usize>().ok())
            .map(ElementId)
            .expect("route action event target");
        set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
        let click = Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>;
        doc.vdom
            .runtime()
            .handle_event("click", Event::new(click, true), id);
        doc.poll(None);
        assert_eq!(
            *ROW_ROUTE_NAVIGATIONS.lock().unwrap(),
            vec![expected.to_string()]
        );
        assert!(
            ROW_ROUTE_EVENTS.lock().unwrap().is_empty(),
            "navigating action also emitted"
        );
    }
}

#[cfg(feature = "native")]
static WIZARD_EVENTS: std::sync::Mutex<Vec<(String, serde_json::Value)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn wizard_callback_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};

    let props: orbital_core::Value = serde_json::from_value(serde_json::json!({
        "type": "wizard-container",
        "steps": [{"id": "one", "title": "One"}, {"id": "two", "title": "Two"}],
        "showProgress": false,
        "nextEvent": "NEXT",
        "completeEvent": "COMPLETE",
        "onStepChange": "STEP_CHANGED",
        "onComplete": "DONE"
    }))
    .unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        WIZARD_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_value(payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "Wizard",
        orbital_name: "Main",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    default_registry().render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn wizard_callback_names_use_the_shared_serialized_event_contract() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use dioxus_core::ElementId;
    use dioxus_native::{DioxusDocument, DocumentConfig};
    use std::{any::Any, rc::Rc};

    WIZARD_EVENTS.lock().unwrap().clear();
    let mut doc = DioxusDocument::new(
        VirtualDom::new(wizard_callback_app),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc.poll(None);
    let click_last_button = |doc: &mut DioxusDocument| {
        let button = *doc
            .query_selector_all("button")
            .unwrap()
            .last()
            .expect("wizard button");
        let dioxus_id = doc
            .get_node(button)
            .and_then(|node| node.element_data())
            .and_then(|element| {
                element
                    .attrs
                    .iter()
                    .find(|attr| *attr.name.local == *"data-dioxus-id")
            })
            .and_then(|attr| attr.value.parse::<usize>().ok())
            .map(ElementId)
            .expect("wizard button event target");
        let mouse = Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>;
        set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
        doc.vdom
            .runtime()
            .handle_event("click", Event::new(mouse, true), dioxus_id);
        doc.poll(None);
    };

    click_last_button(&mut doc);
    click_last_button(&mut doc);
    assert_eq!(
        *WIZARD_EVENTS.lock().unwrap(),
        vec![
            ("NEXT".into(), serde_json::json!({"step": 1})),
            ("STEP_CHANGED".into(), serde_json::json!({"stepIndex": 1})),
            ("COMPLETE".into(), serde_json::json!({"step": 1})),
            ("DONE".into(), serde_json::Value::Null),
        ]
    );
}

#[cfg(feature = "native")]
static ATOM_CALLBACK_PATTERN: std::sync::Mutex<&str> = std::sync::Mutex::new("");
#[cfg(feature = "native")]
static ATOM_CALLBACK_EVENTS: std::sync::Mutex<Vec<(String, serde_json::Value)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(feature = "native")]
fn callback_atom_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};

    let props: orbital_core::Value =
        serde_json::from_str(*ATOM_CALLBACK_PATTERN.lock().unwrap()).unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        ATOM_CALLBACK_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_value(payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "Atom",
        orbital_name: "Main",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    default_registry().render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn flip_and_badge_callbacks_emit_their_declared_empty_events() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use dioxus_core::ElementId;
    use dioxus_native::{DioxusDocument, DocumentConfig};
    use std::{any::Any, rc::Rc};

    let click = |pattern: &'static str, selector: &str| {
        *ATOM_CALLBACK_PATTERN.lock().unwrap() = pattern;
        ATOM_CALLBACK_EVENTS.lock().unwrap().clear();
        let mut doc = DioxusDocument::new(
            VirtualDom::new(callback_atom_app),
            DocumentConfig::default(),
        );
        doc.initial_build();
        doc.poll(None);
        let target = doc
            .query_selector(selector)
            .unwrap()
            .expect("interactive atom");
        let dioxus_id = doc
            .get_node(target)
            .and_then(|node| node.element_data())
            .and_then(|element| {
                element
                    .attrs
                    .iter()
                    .find(|attr| *attr.name.local == *"data-dioxus-id")
            })
            .and_then(|attr| attr.value.parse::<usize>().ok())
            .map(ElementId)
            .expect("atom click target");
        let mouse = Rc::new(PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default())) as Rc<dyn Any>;
        set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
        doc.vdom
            .runtime()
            .handle_event("click", Event::new(mouse, true), dioxus_id);
        doc.poll(None);
        ATOM_CALLBACK_EVENTS.lock().unwrap().clone()
    };
    assert_eq!(
        click(
            r#"{"type":"flip-container","flipped":false,"onClick":"FLIP","children":[]}"#,
            "[role='button']"
        ),
        vec![("FLIP".into(), serde_json::Value::Null)]
    );
    assert_eq!(
        click(
            r#"{"type":"badge","label":"Todo","onRemove":"REMOVE","removeLabel":"Remove Todo"}"#,
            "button[aria-label='Remove Todo']"
        ),
        vec![("REMOVE".into(), serde_json::Value::Null)]
    );
}

#[cfg(feature = "native")]
thread_local! {
    static PRESENCE_SHOW: std::cell::RefCell<Option<dioxus::prelude::Signal<bool>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "native")]
fn presence_callback_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};

    let show = use_signal(|| true);
    PRESENCE_SHOW.with(|slot| *slot.borrow_mut() = Some(show));
    let props: orbital_core::Value = serde_json::from_value(serde_json::json!({
        "type": "presence",
        "show": show(),
        "animate": false,
        "onExited": "EXIT",
        "children": [{"type": "typography", "content": "Present"}]
    }))
    .unwrap();
    let entities = std::collections::HashMap::new();
    let trait_content = orbital_core::ValueMap::new();
    let sink = Callback::new(|(event, payload): (String, orbital_core::Value)| {
        ATOM_CALLBACK_EVENTS
            .lock()
            .unwrap()
            .push((event, serde_json::to_value(payload).unwrap()));
    });
    let view = DioxusView {
        trait_name: "Presence",
        orbital_name: "Main",
        current_state: "ready",
        entities: &entities,
        trait_content: &trait_content,
        source_trait: None,
        sink: Some(sink),
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    default_registry().render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn presence_and_typewriter_completion_emit_declared_empty_events() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    use blitz_dom::Document as _;
    use dioxus::prelude::*;
    use dioxus_native::{DioxusDocument, DocumentConfig};

    ATOM_CALLBACK_EVENTS.lock().unwrap().clear();
    let mut presence = DioxusDocument::new(
        VirtualDom::new(presence_callback_app),
        DocumentConfig::default(),
    );
    presence.initial_build();
    presence.poll(None);
    assert!(ATOM_CALLBACK_EVENTS.lock().unwrap().is_empty());
    PRESENCE_SHOW.with(|slot| {
        let mut show = (*slot.borrow()).expect("presence signal");
        show.set(false);
    });
    presence.poll(None);
    presence.poll(None);
    assert_eq!(
        *ATOM_CALLBACK_EVENTS.lock().unwrap(),
        vec![("EXIT".into(), serde_json::Value::Null)]
    );

    *ATOM_CALLBACK_PATTERN.lock().unwrap() =
        r#"{"type":"typewriter-text","text":"A","speed":0,"onComplete":"TYPE_DONE"}"#;
    ATOM_CALLBACK_EVENTS.lock().unwrap().clear();
    let mut typewriter = DioxusDocument::new(
        VirtualDom::new(callback_atom_app),
        DocumentConfig::default(),
    );
    typewriter.initial_build();
    typewriter.poll(None);
    assert_eq!(
        *ATOM_CALLBACK_EVENTS.lock().unwrap(),
        vec![("TYPE_DONE".into(), serde_json::Value::Null)]
    );
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
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
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
      "traitView":{"lens":"flow","filter":null}, "onFlowSelect":"FLOW_CALLBACK", "flowSelectEvent":"SELECT_FLOW", "onTraitViewChange":"VIEW_CHANGED",
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
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(feature = "native")]
#[test]
fn flow_canvas_selects_a_core_grouped_unit() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "FLOW_CALLBACK");
    assert_eq!(events[1].0, "SELECT_FLOW");
    let callback: serde_json::Value = serde_json::from_str(&events[0].1).unwrap();
    let event: serde_json::Value = serde_json::from_str(&events[1].1).unwrap();
    assert_eq!(callback["unit"]["id"], "behavior:flow");
    assert_eq!(
        callback["unit"]["traits"],
        serde_json::json!(["One", "Two"])
    );
    assert_eq!(callback["unit"], event);
    drop(events);
    let show_traits = selected
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" && *id != button => {
                Some(*id)
            }
            _ => None,
        })
        .expect("show traits action after flow selection");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    dom.runtime()
        .handle_event("click", Event::new(mouse, true), show_traits);
    let events = FLOW_EVENTS.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2].0, "VIEW_CHANGED");
    let view: serde_json::Value = serde_json::from_str(&events[2].1).unwrap();
    assert_eq!(
        view,
        serde_json::json!({"view":{"lens":"traits","filter":{"unit":"flow","traits":["One","Two"]}}})
    );
}

#[cfg(feature = "native")]
#[test]
fn atomic_controls_emit_the_declared_event_payloads() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
            r#"{"type":"input","label":"New task","onChange":"DRAFT_CHANGED"}"#,
            "input",
            form
        ),
        vec![(
            "DRAFT_CHANGED".to_string(),
            r#"{"value":"Buy milk"}"#.to_string()
        )]
    );
    let enter = || {
        Rc::new(PlatformEventData::new(Box::new(
            dioxus_html::SerializedKeyboardData::new(
                dioxus_html::Key::Enter,
                dioxus_html::Code::Enter,
                dioxus_html::Location::Standard,
                false,
                dioxus_html::Modifiers::empty(),
                false,
            ),
        ))) as Rc<dyn Any>
    };
    assert_eq!(
        dispatch(
            r#"{"type":"input","label":"New task","value":"Buy milk","action":"SUBMIT_TASK"}"#,
            "keydown",
            enter(),
        ),
        vec![(
            "SUBMIT_TASK".to_string(),
            r#"{"value":"Buy milk"}"#.to_string(),
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
    *CONTROL_PATTERN.lock().unwrap() =
        r#"{"type":"checkbox","label":"Complete","checked":false,"onChange":"SET_DONE"}"#;
    CONTROL_EVENTS.lock().unwrap().clear();
    let mut label_dom = VirtualDom::new(control_app);
    let mut label_mutations = Mutations::default();
    label_dom.rebuild(&mut label_mutations);
    let click_listeners: Vec<_> = label_mutations
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(click_listeners.len(), 2);
    label_dom.runtime().handle_event(
        "click",
        Event::new(mouse(), true),
        *click_listeners.last().unwrap(),
    );
    assert_eq!(
        *CONTROL_EVENTS.lock().unwrap(),
        vec![("SET_DONE".to_string(), r#"{"checked":true}"#.to_string())]
    );
    let disabled_form = Rc::new(PlatformEventData::new(Box::new(
        dioxus_html::SerializedFormData::new("Ignored".to_string(), vec![]),
    ))) as Rc<dyn Any>;
    assert!(dispatch(
        r#"{"type":"input","label":"New task","onChange":"DRAFT_CHANGED","disabled":true}"#,
        "input",
        disabled_form
    )
    .is_empty());
    let number_form = Rc::new(PlatformEventData::new(Box::new(
        dioxus_html::SerializedFormData::new("42".to_string(), vec![]),
    ))) as Rc<dyn Any>;
    assert_eq!(
        dispatch(
            r#"{"type":"input","label":"Estimate","inputType":"number","onChange":"ESTIMATE_TYPED"}"#,
            "input",
            number_form
        ),
        vec![(
            "ESTIMATE_TYPED".to_string(),
            r#"{"value":"42"}"#.to_string()
        )]
    );
    assert_eq!(
        dispatch(
            r#"{"type":"input","label":"Estimate","inputType":"number","value":"42","action":"ESTIMATE_CHANGED"}"#,
            "keydown",
            enter(),
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
static EVENT_CONVERTER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    registry.register(Box::new(game::molecules::action_tile::ActionTileRenderer));
    registry.register(Box::new(game::molecules::game_menu::GameMenuRenderer));
    registry.register(Box::new(game::molecules::trait_slot::TraitSlotRenderer));
    registry.register(Box::new(game::molecules::sequence_bar::SequenceBarRenderer));
    registry.register(Box::new(game::templates::game_shell::GameShellRenderer));
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
        source_sink: Callback::new(move |_: Option<String>| sink),
        navigate: Some(navigate),
        dock: None,
        viewport_width,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
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
fn new_control_click_at(pattern: &'static str, index: usize) -> Vec<(String, String)> {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
    *NEW_CONTROL_PATTERN.lock().unwrap() = pattern;
    NEW_CONTROL_EVENTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(new_control_app);
    let mut mutations = Mutations::default();
    dom.rebuild(&mut mutations);
    let id = mutations
        .edits
        .iter()
        .filter_map(|edit| match edit {
            Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
            _ => None,
        })
        .nth(index)
        .expect("requested slot click listener");
    let mouse = Rc::new(PlatformEventData::new(Box::<
        dioxus_html::SerializedMouseData,
    >::default())) as Rc<dyn Any>;
    dom.runtime()
        .handle_event("click", Event::new(mouse, true), id);
    NEW_CONTROL_EVENTS.lock().unwrap().clone()
}

#[cfg(feature = "native")]
#[test]
fn authored_aria_pressed_overrides_game_atom_defaults() {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for pattern in [
        r#"{"type":"choice-button","text":"Choice","selected":true,"aria-pressed":"mixed"}"#,
        r#"{"type":"game-audio-toggle","muted":true,"soundOnLabel":"Sound on","mutedLabel":"Muted","aria-pressed":"mixed"}"#,
    ] {
        *NEW_CONTROL_PATTERN.lock().unwrap() = pattern;
        let mut dom = VirtualDom::new(new_control_app);
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        let pressed: Vec<_> = mutations
            .edits
            .iter()
            .filter_map(|edit| match edit {
                Mutation::SetAttribute {
                    name: "aria-pressed",
                    value: AttributeValue::Text(value),
                    ..
                } => Some(value.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(pressed, ["mixed"]);
    }
}

#[cfg(feature = "native")]
#[test]
fn action_tile_renders_authored_drag_and_category_presentation() {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for (disabled, draggable, color) in [
        (false, true, "background:#112233"),
        (true, false, "background:var(--almadar-bg)"),
    ] {
        *NEW_CONTROL_PATTERN.lock().unwrap() = if disabled {
            r##"{"type":"action-tile","action":{"id":"jump","name":"Jump","category":"move"},"disabled":true,"categoryColors":{"move":{"bg":"#112233","border":"#445566"}}}"##
        } else {
            r##"{"type":"action-tile","action":{"id":"jump","name":"Jump","category":"move"},"categoryColors":{"move":{"bg":"#112233","border":"#445566"}}}"##
        };
        let mut dom = VirtualDom::new(new_control_app);
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        assert!(mutations.edits.iter().any(|edit| matches!(edit,
            Mutation::SetAttribute { name: "draggable", value: AttributeValue::Bool(value), .. }
                if *value == draggable
        )));
        assert!(mutations.edits.iter().any(|edit| matches!(edit,
            Mutation::SetAttribute { name: "style", value: AttributeValue::Text(value), .. }
                if value.contains(color)
        )));
    }
}

#[cfg(feature = "native")]
#[test]
fn game_shell_and_action_tile_keep_authored_accessibility_attrs() {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for (pattern, label, role) in [
        (
            r#"{"type":"game-shell","appName":"Snake","showTopBar":false,"aria-label":"Game surface","role":"application","lang":"ar","dir":"rtl","tabIndex":0}"#,
            "Game surface",
            "application",
        ),
        (
            r#"{"type":"action-tile","action":{"id":"jump","name":"Jump","category":"move"},"aria-label":"Jump action","role":"button","lang":"ar","dir":"rtl","tabIndex":0}"#,
            "Jump action",
            "button",
        ),
    ] {
        *NEW_CONTROL_PATTERN.lock().unwrap() = pattern;
        let mut dom = VirtualDom::new(new_control_app);
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        for (name, expected) in [("role", role), ("lang", "ar"), ("dir", "rtl")] {
            assert!(mutations.edits.iter().any(|edit| matches!(edit,
                Mutation::SetAttribute { name: actual, value: AttributeValue::Text(value), .. }
                    if *actual == name && value == expected
            )));
        }
        assert!(mutations.edits.iter().any(|edit| matches!(
            edit,
            Mutation::SetAttribute {
                name: "tabindex",
                value: AttributeValue::Int(0),
                ..
            }
        )));
        assert!(mutations.edits.iter().any(|edit| matches!(edit,
            Mutation::SetAttribute { name: "aria-label", value: AttributeValue::Text(value), .. }
                if value == label
        )));
    }
}

#[cfg(feature = "native")]
#[test]
fn game_shell_null_slots_do_not_create_empty_overlays() {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *NEW_CONTROL_PATTERN.lock().unwrap() = r#"{"type":"game-shell","showTopBar":false,"hud":null,"controls":null,"addons":null,"overlay":null,"data-theme":""}"#;
    let mut dom = VirtualDom::new(new_control_app);
    let mut mutations = Mutations::default();
    dom.rebuild(&mut mutations);
    for class in [
        "game-shell__top",
        "game-shell__hud",
        "game-shell__controls",
        "game-shell__actions",
        "game-shell__overlay",
    ] {
        assert!(!mutations.edits.iter().any(|edit| matches!(edit,
            Mutation::SetAttribute { name: "class", value: AttributeValue::Text(value), .. }
                if value.contains(class)
        )));
    }
    assert!(mutations.edits.iter().all(|edit| match edit {
        Mutation::SetAttribute {
            name: "data-theme",
            value,
            ..
        } => {
            matches!(value, AttributeValue::None)
                || matches!(value, AttributeValue::Text(text) if text.is_empty())
        }
        _ => true,
    }));
}

#[cfg(feature = "native")]
#[test]
fn compact_activity_rail_opens_its_authored_panel_after_selection() {
    use dioxus::prelude::*;
    use dioxus_core::{Mutation, Mutations};
    use std::{any::Any, rc::Rc};

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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

    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert_eq!(
        new_control_events(
            r#"{"type":"choice-button","text":"Open","action":"CHOOSE","payload":{"nextId":"scene-2"},"onClick":"CALLBACK"}"#,
            &["click"]
        ),
        vec![
            ("CHOOSE".into(), r#"{"nextId":"scene-2"}"#.into()),
            ("CALLBACK".into(), "{}".into()),
        ]
    );
    assert_eq!(
        new_control_events(
            r#"{"type":"choice-button","text":"Open","action":"CHOOSE"}"#,
            &["click"]
        ),
        vec![("CHOOSE".into(), "{}".into())]
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
            ("FIRE".into(), "{}".into()),
            ("CALLBACK".into(), "{}".into()),
            ("STOP".into(), "{}".into()),
            ("CALLBACK".into(), "{}".into()),
        ]
    );
    assert_eq!(
        new_control_events(button, &["mousedown", "mouseleave", "click"]),
        vec![
            ("FIRE".into(), "{}".into()),
            ("CALLBACK".into(), "{}".into()),
            ("STOP".into(), "{}".into()),
            ("CALLBACK".into(), "{}".into()),
        ]
    );
}

#[cfg(feature = "native")]
#[test]
fn game_audio_toggle_emits_authored_next_state_only() {
    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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

#[cfg(feature = "native")]
#[test]
fn trait_slot_and_sequence_bar_dispatch_distinct_authored_click_callbacks() {
    let _guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert_eq!(
        new_control_click_at(
            r#"{"type":"trait-slot","slotNumber":2,"emptyLabel":"Empty","clickEvent":"CLICK","onClick":"CALLBACK"}"#,
            0,
        ),
        vec![
            ("CLICK".into(), r#"{"slotNumber":2.0}"#.into()),
            ("CALLBACK".into(), "null".into()),
        ]
    );
    assert_eq!(
        new_control_click_at(
            r#"{"type":"sequence-bar","maxSlots":1,"slots":[{"id":"jump","name":"Jump","category":"move"}],"slotRemoveEvent":"REMOVE","onSlotRemove":"CALLBACK","removeLabel":"Remove"}"#,
            1,
        ),
        vec![
            ("REMOVE".into(), r#"{"slotNumber":1.0}"#.into()),
            ("CALLBACK".into(), r#"{"index":1.0}"#.into()),
        ]
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
        source_sink: dioxus::prelude::Callback::new(|_: Option<String>| {
            dioxus::prelude::Callback::new(|_: (String, orbital_core::Value)| {})
        }),
        navigate: None,
        dock: None,
        viewport_width: None,
        canvas_theme: None,
        theme_mode: None,
        toggle_theme: None,
    };
    registry.render_spec(&props, None, &view)
}

#[cfg(all(feature = "native", feature = "research-drag"))]
#[test]
fn trait_slot_empty_remove_event_does_not_add_a_remove_control() {
    let _converter_guard = EVENT_CONVERTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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

fn search_input_placeholder(locale: &str) -> String {
    use dioxus::prelude::*;
    use dioxus_core::{AttributeValue, Mutation, Mutations};
    use orbital_client_dioxus::{i18n, registry::default_registry, DioxusView};

    fn app() -> Element {
        let spec: orbital_core::Value = serde_json::from_str(r#"{"type":"search-input"}"#).unwrap();
        let entities = std::collections::HashMap::new();
        let trait_content = orbital_core::ValueMap::new();
        let view = DioxusView {
            trait_name: "Find",
            orbital_name: "Main",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: None,
            source_sink: Callback::new(|_: Option<String>| Callback::new(|_: (String, orbital_core::Value)| {})),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        default_registry().render_spec(&spec, None, &view)
    }

    i18n::set_locale(Some(locale.to_string()), Default::default());
    let mut dom = VirtualDom::new(app);
    let mut initial = Mutations::default();
    dom.rebuild(&mut initial);
    initial
        .edits
        .iter()
        .find_map(|edit| match edit {
            Mutation::SetAttribute { name: "placeholder", value: AttributeValue::Text(text), .. } => Some(text.clone()),
            _ => None,
        })
        .expect("the search input paints a placeholder")
}

#[test]
fn a_std_pattern_paints_the_locale_catalog_text() {
    let arabic = orbital_core::i18n::ui::ui_message("ar", "common.search");
    assert_ne!(arabic, "Search...");
    assert_eq!(search_input_placeholder("ar"), arabic);
}

#[test]
fn control_en_paints_english_and_an_unshipped_locale_falls_back_to_it() {
    assert_eq!(search_input_placeholder("en"), "Search...");
    assert_eq!(search_input_placeholder("fr"), "Search...");
}
