use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

struct DraftState<T> {
    incoming: T,
    pending: Vec<T>,
}

impl<T: Clone + PartialEq> DraftState<T> {
    fn accept(&mut self, incoming: T) -> Option<T> {
        // G-CROSS-415: pending echoes acknowledge edits; external values replace the draft.
        if self.incoming == incoming {
            return None;
        }
        self.incoming = incoming.clone();
        if let Some(index) = self.pending.iter().position(|pending| *pending == incoming) {
            self.pending.remove(index);
            None
        } else {
            self.pending.clear();
            Some(incoming)
        }
    }

    fn record(&mut self, next: T) {
        self.pending.push(next);
    }
}

#[derive(Clone)]
pub(super) struct FieldDraft<T: Clone + PartialEq + 'static> {
    value: Signal<T>,
    state: Rc<RefCell<DraftState<T>>>,
    immediate: Option<T>,
}

impl<T: Clone + PartialEq + 'static> FieldDraft<T> {
    pub(super) fn value(&self) -> T {
        let current = self.value.read().clone();
        self.immediate.clone().unwrap_or(current)
    }

    pub(super) fn signal(&self) -> ReadSignal<T> {
        ReadSignal::new(self.value)
    }

    pub(super) fn record(&mut self, next: T) {
        self.state.borrow_mut().record(next.clone());
        self.immediate = None;
        self.value.set(next);
    }
}

pub(super) fn use_field_draft<T: Clone + PartialEq + 'static>(incoming: T) -> FieldDraft<T> {
    let state = use_hook(|| {
        Rc::new(RefCell::new(DraftState {
            incoming: incoming.clone(),
            pending: Vec::new(),
        }))
    });
    let mut value = use_signal(|| incoming.clone());
    let immediate = state.borrow_mut().accept(incoming);
    if let Some(external) = &immediate {
        value.set(external.clone());
    }
    FieldDraft {
        value,
        state,
        immediate,
    }
}

#[cfg(test)]
mod tests {
    use super::{use_field_draft, DraftState};
    use dioxus::prelude::*;
    use dioxus_core::{Mutations, ScopeId, VirtualDom};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Clone)]
    struct FieldCase {
        id: String,
        incoming: String,
        edit: Option<String>,
    }

    thread_local! {
        static CASES: RefCell<Vec<FieldCase>> = const { RefCell::new(Vec::new()) };
        static PAINTED: RefCell<Vec<(String, String, String, usize)>> = const { RefCell::new(Vec::new()) };
        static NEXT_SCOPE: Cell<usize> = const { Cell::new(0) };
    }

    #[component]
    fn ProbeField(id: String, incoming: String, edit: Option<String>) -> Element {
        let scope = use_hook(|| {
            NEXT_SCOPE.with(|next| {
                let id = next.get();
                next.set(id + 1);
                id
            })
        });
        let mut draft = use_field_draft(incoming.clone());
        let last_edit = use_hook(|| Rc::new(RefCell::new(None::<String>)));
        if *last_edit.borrow() != edit {
            *last_edit.borrow_mut() = edit.clone();
            if let Some(edit) = edit.clone() {
                draft.record(edit);
            }
        }
        let value = draft.value();
        PAINTED.with(|painted| {
            painted
                .borrow_mut()
                .push((id.clone(), incoming, value.clone(), scope))
        });
        rsx! { input { id, value } }
    }

    fn probe_app() -> Element {
        let cases = CASES.with(|cases| cases.borrow().clone());
        rsx! {
            for case in cases {
                ProbeField { key: "{case.id}", id: case.id, incoming: case.incoming, edit: case.edit }
            }
        }
    }

    fn latest(id: &str) -> String {
        PAINTED.with(|painted| {
            painted
                .borrow()
                .iter()
                .rev()
                .find(|(field, _, _, _)| field == id)
                .expect("field painted")
                .2
                .clone()
        })
    }

    fn latest_incoming(id: &str) -> String {
        PAINTED.with(|painted| {
            painted
                .borrow()
                .iter()
                .rev()
                .find(|(field, _, _, _)| field == id)
                .expect("field painted")
                .1
                .clone()
        })
    }

    fn latest_scope(id: &str) -> usize {
        PAINTED.with(|painted| {
            painted
                .borrow()
                .iter()
                .rev()
                .find(|(field, _, _, _)| field == id)
                .expect("field painted")
                .3
        })
    }

    #[test]
    fn mounted_fields_keep_drafts_when_reordered_and_echoes_arrive_late() {
        CASES.with(|cases| {
            *cases.borrow_mut() = vec![
                FieldCase {
                    id: "first".into(),
                    incoming: String::new(),
                    edit: None,
                },
                FieldCase {
                    id: "second".into(),
                    incoming: String::new(),
                    edit: None,
                },
            ];
        });
        PAINTED.with(|painted| painted.borrow_mut().clear());
        let mut dom = VirtualDom::new(probe_app);
        dom.rebuild(&mut Mutations::default());
        let first_scope = latest_scope("first");
        CASES.with(|cases| {
            cases.borrow_mut()[0].edit = Some("A".into());
        });
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut Mutations::default());
        assert_eq!(latest_incoming("first"), "");
        assert_eq!(latest("first"), "A");
        CASES.with(|cases| {
            cases.borrow_mut()[0].edit = Some("AB".into());
        });
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut Mutations::default());
        assert_eq!(latest("first"), "AB");
        CASES.with(|cases| {
            let mut cases = cases.borrow_mut();
            cases[0].incoming = "A".into();
            cases.swap(0, 1);
        });
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut Mutations::default());
        assert_eq!(latest_incoming("first"), "A");
        assert_eq!(latest_scope("first"), first_scope);
        assert_eq!(latest("first"), "AB");
        assert_eq!(latest("second"), "");
        CASES.with(|cases| {
            cases.borrow_mut()[1].incoming = "AB".into();
        });
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut Mutations::default());
        assert_eq!(latest_incoming("first"), "AB");
        assert_eq!(latest("first"), "AB");
        CASES.with(|cases| {
            cases.borrow_mut()[1].incoming = String::new();
        });
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut Mutations::default());
        assert_eq!(latest_incoming("first"), "");
        assert_eq!(latest_scope("first"), first_scope);
        assert_eq!(latest("first"), "");
    }

    #[test]
    fn delayed_echoes_preserve_rapid_edits_until_external_clear() {
        let mut state = DraftState {
            incoming: String::new(),
            pending: Vec::new(),
        };
        state.record("A".to_string());
        state.record("AB".to_string());
        assert_eq!(state.accept("A".to_string()), None);
        assert_eq!(state.accept("AB".to_string()), None);
        assert_eq!(state.accept(String::new()), Some(String::new()));
    }

    #[test]
    fn multi_select_values_follow_the_same_echo_policy() {
        let mut state = DraftState {
            incoming: Vec::<String>::new(),
            pending: Vec::new(),
        };
        state.record(vec!["one".to_string()]);
        state.record(vec!["one".to_string(), "two".to_string()]);
        assert_eq!(state.accept(vec!["one".to_string()]), None);
        assert_eq!(
            state.accept(vec!["one".to_string(), "two".to_string()]),
            None
        );
        assert_eq!(state.accept(Vec::new()), Some(Vec::new()));
    }
}
