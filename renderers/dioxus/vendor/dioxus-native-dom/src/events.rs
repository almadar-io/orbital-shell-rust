use blitz_traits::events::{
    BlitzDragEvent, BlitzDragTransfer, BlitzKeyEvent, BlitzMouseButtonEvent, BlitzTouchEvent,
    BlitzTouchPoint, BlitzWheelEvent, MouseEventButton, WheelDeltaMode,
};
use dioxus_html::{
    geometry::{ClientPoint, ElementPoint, PagePoint, ScreenPoint, WheelDelta},
    input_data::{MouseButton, MouseButtonSet},
    point_interaction::{
        InteractionElementOffset, InteractionLocation, ModifiersInteraction, PointerInteraction,
    },
    AnimationData, CancelData, ClipboardData, CompositionData, DataTransfer, DragData, FocusData,
    FormData, FormValue, HasDataTransferData, HasDragData, HasFileData, HasFocusData, HasFormData,
    HasImageData, HasKeyboardData, HasMouseData, HasTouchData, HasTouchPointData, HasWheelData,
    HtmlEventConverter, ImageData, KeyboardData,
    MediaData, MountedData, MouseData, NativeDataTransfer, PlatformEventData, PointerData,
    ResizeData, ScrollData, SelectionData, ToggleData, TouchData, TransitionData, VisibleData,
    WheelData,
};
use keyboard_types::{Code, Key, Location, Modifiers};
use std::any::Any;

pub struct NativeConverter {}

impl HtmlEventConverter for NativeConverter {
    fn convert_cancel_data(&self, _event: &PlatformEventData) -> CancelData {
        unimplemented!("todo: convert_cancel_data in dioxus-native. requires support in blitz")
    }

    fn convert_form_data(&self, event: &PlatformEventData) -> FormData {
        event.downcast::<NativeFormData>().unwrap().clone().into()
    }

    fn convert_mouse_data(&self, event: &PlatformEventData) -> MouseData {
        event.downcast::<NativeClickData>().unwrap().clone().into()
    }

    fn convert_keyboard_data(&self, event: &PlatformEventData) -> KeyboardData {
        event
            .downcast::<BlitzKeyboardData>()
            .unwrap()
            .clone()
            .into()
    }

    fn convert_focus_data(&self, _event: &PlatformEventData) -> FocusData {
        NativeFocusData {}.into()
    }

    fn convert_animation_data(&self, _event: &PlatformEventData) -> AnimationData {
        unimplemented!("todo: convert_animation_data in dioxus-native. requires support in blitz")
    }

    fn convert_clipboard_data(&self, _event: &PlatformEventData) -> ClipboardData {
        unimplemented!("todo: convert_clipboard_data in dioxus-native. requires support in blitz")
    }

    fn convert_composition_data(&self, _event: &PlatformEventData) -> CompositionData {
        unimplemented!("todo: convert_composition_data in dioxus-native. requires support in blitz")
    }

    fn convert_drag_data(&self, event: &PlatformEventData) -> DragData {
        event.downcast::<NativeDragData>().unwrap().clone().into()
    }

    fn convert_image_data(&self, event: &PlatformEventData) -> ImageData {
        event.downcast::<NativeImageData>().unwrap().clone().into()
    }

    fn convert_media_data(&self, _event: &PlatformEventData) -> MediaData {
        unimplemented!("todo: convert_media_data in dioxus-native. requires support in blitz")
    }

    fn convert_mounted_data(&self, _event: &PlatformEventData) -> MountedData {
        unimplemented!("todo: convert_mounted_data in dioxus-native. requires support in blitz")
    }

    fn convert_pointer_data(&self, _event: &PlatformEventData) -> PointerData {
        unimplemented!("todo: convert_pointer_data in dioxus-native. requires support in blitz")
    }

    fn convert_scroll_data(&self, _event: &PlatformEventData) -> ScrollData {
        unimplemented!("todo: convert_scroll_data in dioxus-native. requires support in blitz")
    }

    fn convert_selection_data(&self, _event: &PlatformEventData) -> SelectionData {
        unimplemented!("todo: convert_selection_data in dioxus-native. requires support in blitz")
    }

    fn convert_toggle_data(&self, _event: &PlatformEventData) -> ToggleData {
        unimplemented!("todo: convert_toggle_data in dioxus-native. requires support in blitz")
    }

    // ORBITAL PATCH: multi-touch
    fn convert_touch_data(&self, event: &PlatformEventData) -> TouchData {
        event.downcast::<NativeTouchData>().unwrap().clone().into()
    }

    fn convert_transition_data(&self, _event: &PlatformEventData) -> TransitionData {
        unimplemented!("todo: convert_transition_data in dioxus-native. requires support in blitz")
    }

    // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
    fn convert_wheel_data(&self, event: &PlatformEventData) -> WheelData {
        event.downcast::<NativeWheelData>().unwrap().clone().into()
    }

    fn convert_resize_data(&self, _event: &PlatformEventData) -> ResizeData {
        unimplemented!("todo: convert_resize_data in dioxus-native. requires support in blitz")
    }

    fn convert_visible_data(&self, _event: &PlatformEventData) -> VisibleData {
        unimplemented!("todo: convert_visible_data in dioxus-native. requires support in blitz")
    }
}

// ORBITAL PATCH: multi-touch
/// A touch event plus where its target element sits, so a handler can read each finger inside the element.
#[derive(Clone)]
pub struct NativeTouchData {
    pub(crate) event: BlitzTouchEvent,
    pub(crate) origin: (f64, f64),
}

// ORBITAL PATCH: multi-touch
#[derive(Clone)]
pub struct NativeTouchPoint {
    point: BlitzTouchPoint,
    origin: (f64, f64),
}

impl NativeTouchPoint {
    /// The platform's id for this finger (dioxus-html keeps `TouchPoint::identifier` private).
    pub fn id(&self) -> u64 {
        self.point.id
    }
}

impl NativeTouchData {
    /// The finger that changed, as `(id, offset inside the target element)`.
    pub fn changed_in_element(&self) -> (u64, ElementPoint) {
        (self.event.changed.id, self.element_point(&self.event.changed))
    }

    /// Every finger still on the screen after the event, as `(id, offset inside the target element)`.
    pub fn touches_in_element(&self) -> Vec<(u64, ElementPoint)> {
        self.event
            .touches
            .iter()
            .map(|touch| (touch.id, self.element_point(touch)))
            .collect()
    }

    fn element_point(&self, touch: &BlitzTouchPoint) -> ElementPoint {
        ElementPoint::new(touch.x as f64 - self.origin.0, touch.y as f64 - self.origin.1)
    }

    fn points(&self, touches: &[BlitzTouchPoint]) -> Vec<dioxus_html::TouchPoint> {
        touches
            .iter()
            .map(|point| NativeTouchPoint { point: *point, origin: self.origin }.into())
            .collect()
    }
}

impl ModifiersInteraction for NativeTouchData {
    fn modifiers(&self) -> Modifiers {
        self.event.mods
    }
}

impl HasTouchData for NativeTouchData {
    fn touches(&self) -> Vec<dioxus_html::TouchPoint> {
        self.points(&self.event.touches)
    }

    fn touches_changed(&self) -> Vec<dioxus_html::TouchPoint> {
        self.points(std::slice::from_ref(&self.event.changed))
    }

    fn target_touches(&self) -> Vec<dioxus_html::TouchPoint> {
        self.points(&self.event.touches)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl InteractionLocation for NativeTouchPoint {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.point.x as _, self.point.y as _)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        ScreenPoint::new(self.point.x as _, self.point.y as _)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.point.x as _, self.point.y as _)
    }
}

impl HasTouchPointData for NativeTouchPoint {
    fn identifier(&self) -> i32 {
        self.point.id as i32
    }

    fn force(&self) -> f64 {
        0.0
    }

    fn radius(&self) -> ScreenPoint {
        ScreenPoint::new(0.0, 0.0)
    }

    fn rotation(&self) -> f64 {
        0.0
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod touch_event_tests {
    use super::*;
    use blitz_traits::events::BlitzTouchPhase;

    fn event(phase: BlitzTouchPhase, changed: (u64, f32, f32), held: &[(u64, f32, f32)]) -> PlatformEventData {
        let point = |(id, x, y): (u64, f32, f32)| BlitzTouchPoint { id, x, y };
        PlatformEventData::new(Box::new(NativeTouchData {
            event: BlitzTouchEvent {
                phase,
                changed: point(changed),
                touches: held.iter().copied().map(point).collect(),
                mods: Default::default(),
            },
            origin: (100.0, 20.0),
        }))
    }

    #[test]
    fn two_fingers_reach_a_dioxus_touch_handler_with_their_ids_and_element_offsets() {
        let data = NativeConverter {}.convert_touch_data(&event(
            BlitzTouchPhase::Move,
            (9, 170.0, 70.0),
            &[(7, 110.0, 30.0), (9, 170.0, 70.0)],
        ));
        let ids: Vec<u64> = data.touches().iter().map(|t| t.downcast::<NativeTouchPoint>().unwrap().id()).collect();
        assert_eq!(ids, [7, 9]);
        assert_eq!(data.touches_changed().len(), 1);
        assert_eq!(data.touches_changed()[0].downcast::<NativeTouchPoint>().unwrap().id(), 9);
        let native = data.downcast::<NativeTouchData>().expect("native touch data");
        let inside = native.touches_in_element();
        assert_eq!((inside[1].1.x, inside[1].1.y), (70.0, 50.0));
        let (id, at) = native.changed_in_element();
        assert_eq!((id, at.x, at.y), (9, 70.0, 50.0));
    }

    #[test]
    fn control_one_finger_is_one_touch_point() {
        let data = NativeConverter {}.convert_touch_data(&event(BlitzTouchPhase::Start, (3, 105.0, 25.0), &[(3, 105.0, 25.0)]));
        assert_eq!(data.touches().len(), 1);
        assert_eq!(data.touches()[0].downcast::<NativeTouchPoint>().unwrap().id(), 3);
    }
}

#[derive(Clone)]
pub struct NativeImageData {
    pub load_error: bool,
}

impl HasImageData for NativeImageData {
    fn load_error(&self) -> bool {
        self.load_error
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod image_event_tests {
    use super::*;

    #[test]
    fn native_image_error_reaches_dioxus_image_data() {
        let failed = PlatformEventData::new(Box::new(NativeImageData { load_error: true }));
        let loaded = PlatformEventData::new(Box::new(NativeImageData { load_error: false }));
        assert!(NativeConverter {}.convert_image_data(&failed).load_error());
        assert!(!NativeConverter {}.convert_image_data(&loaded).load_error());
    }
}

#[derive(Clone, Debug)]
pub struct NativeFormData {
    pub value: String,
    pub values: Vec<(String, FormValue)>,
}

impl HasFormData for NativeFormData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }

    fn value(&self) -> String {
        self.value.clone()
    }

    fn values(&self) -> Vec<(String, FormValue)> {
        self.values.clone()
    }
    fn valid(&self) -> bool {
        // todo: actually implement validation here.
        true
    }
}

impl HasFileData for NativeFormData {
    fn files(&self) -> Vec<dioxus_html::FileData> {
        vec![]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BlitzKeyboardData(pub(crate) BlitzKeyEvent);

impl ModifiersInteraction for BlitzKeyboardData {
    fn modifiers(&self) -> Modifiers {
        self.0.modifiers
    }
}

impl HasKeyboardData for BlitzKeyboardData {
    fn key(&self) -> Key {
        self.0.key.clone()
    }

    fn code(&self) -> Code {
        self.0.code
    }

    fn location(&self) -> Location {
        self.0.location
    }

    fn is_auto_repeating(&self) -> bool {
        self.0.is_auto_repeating
    }

    fn is_composing(&self) -> bool {
        self.0.is_composing
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn Any
    }
}

#[derive(Clone)]
// ORBITAL PATCH: element coordinates on mouse events (the target's offset, as wheel events carry it)
pub struct NativeClickData(pub(crate) BlitzMouseButtonEvent, pub(crate) (f64, f64), pub(crate) (f64, f64));

// ORBITAL PATCH: the target's laid-out size, so a drag over a childless overlay reads its position as a share of it
impl NativeClickData {
    pub fn target_size(&self) -> (f64, f64) {
        self.2
    }
}

impl InteractionLocation for NativeClickData {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.0.x as _, self.0.y as _)
    }

    // these require blitz to pass them along, or a dom rect
    fn screen_coordinates(&self) -> ScreenPoint {
        unimplemented!()
    }

    fn page_coordinates(&self) -> PagePoint {
        unimplemented!()
    }
}

impl InteractionElementOffset for NativeClickData {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(self.1 .0, self.1 .1)
    }
}

impl ModifiersInteraction for NativeClickData {
    fn modifiers(&self) -> Modifiers {
        self.0.mods
    }
}

impl PointerInteraction for NativeClickData {
    fn trigger_button(&self) -> Option<MouseButton> {
        Some(match self.0.button {
            MouseEventButton::Main => MouseButton::Primary,
            MouseEventButton::Auxiliary => MouseButton::Auxiliary,
            MouseEventButton::Secondary => MouseButton::Secondary,
            MouseEventButton::Fourth => MouseButton::Fourth,
            MouseEventButton::Fifth => MouseButton::Fifth,
        })
    }

    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(self.0.buttons.bits() as u16)
    }
}
impl HasMouseData for NativeClickData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}

// ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
/// A wheel event plus the pointer's offset inside the element it landed on.
#[derive(Clone)]
pub struct NativeWheelData {
    pub(crate) event: BlitzWheelEvent,
    pub(crate) element: (f64, f64),
}

impl InteractionLocation for NativeWheelData {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.event.x as _, self.event.y as _)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        ScreenPoint::new(self.event.x as _, self.event.y as _)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.event.x as _, self.event.y as _)
    }
}

impl InteractionElementOffset for NativeWheelData {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(self.element.0, self.element.1)
    }
}

impl ModifiersInteraction for NativeWheelData {
    fn modifiers(&self) -> Modifiers {
        self.event.mods
    }
}

impl PointerInteraction for NativeWheelData {
    fn trigger_button(&self) -> Option<MouseButton> {
        None
    }

    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(self.event.buttons.bits() as u16)
    }
}

impl HasMouseData for NativeWheelData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}

impl HasWheelData for NativeWheelData {
    fn delta(&self) -> WheelDelta {
        let e = &self.event;
        match e.delta_mode {
            WheelDeltaMode::Pixel => WheelDelta::pixels(e.delta_x, e.delta_y, 0.0),
            WheelDeltaMode::Line => WheelDelta::lines(e.delta_x, e.delta_y, 0.0),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}

#[derive(Clone)]
pub struct NativeDragData(pub(crate) BlitzDragEvent);

impl InteractionLocation for NativeDragData {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.0.mouse.x as _, self.0.mouse.y as _)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.0.mouse.x as _, self.0.mouse.y as _)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        ScreenPoint::new(self.0.mouse.x as _, self.0.mouse.y as _)
    }
}

impl InteractionElementOffset for NativeDragData {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(self.0.mouse.x as _, self.0.mouse.y as _)
    }
}

impl ModifiersInteraction for NativeDragData {
    fn modifiers(&self) -> Modifiers {
        self.0.mouse.mods
    }
}

impl PointerInteraction for NativeDragData {
    fn trigger_button(&self) -> Option<MouseButton> {
        Some(MouseButton::Primary)
    }

    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(self.0.mouse.buttons.bits() as u16)
    }
}

impl HasMouseData for NativeDragData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl HasFileData for NativeDragData {
    fn files(&self) -> Vec<dioxus_html::FileData> {
        Vec::new()
    }
}

impl HasDataTransferData for NativeDragData {
    fn data_transfer(&self) -> DataTransfer {
        DataTransfer::new(NativeDragTransfer(self.0.transfer.clone()))
    }
}

impl HasDragData for NativeDragData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct NativeDragTransfer(BlitzDragTransfer);

impl NativeDataTransfer for NativeDragTransfer {
    fn get_data(&self, mime: &str) -> Option<String> {
        self.0.get_data(mime)
    }

    fn set_data(&self, mime: &str, data: &str) -> Result<(), String> {
        self.0.set_data(mime, data)
    }

    fn clear_data(&self, mime: Option<&str>) -> Result<(), String> {
        self.0.clear_data(mime)
    }

    fn effect_allowed(&self) -> String {
        self.0.effect_allowed()
    }

    fn set_effect_allowed(&self, effect: &str) {
        self.0.set_effect_allowed(effect)
    }

    fn drop_effect(&self) -> String {
        self.0.drop_effect()
    }

    fn set_drop_effect(&self, effect: &str) {
        self.0.set_drop_effect(effect)
    }

    fn files(&self) -> Vec<dioxus_html::FileData> {
        Vec::new()
    }
}

#[derive(Clone)]
pub struct NativeFocusData {}
impl HasFocusData for NativeFocusData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}
