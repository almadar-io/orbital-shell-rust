//! Types to represent UI and DOM events

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bitflags::bitflags;
use keyboard_types::{Code, Key, Location, Modifiers};
use smol_str::SmolStr;

#[derive(Default)]
pub struct EventState {
    cancelled: bool,
    propagation_stopped: bool,
    redraw_requested: bool,
}
impl EventState {
    #[inline(always)]
    pub fn prevent_default(&mut self) {
        self.cancelled = true;
    }

    #[inline(always)]
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    #[inline(always)]
    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    #[inline(always)]
    pub fn propagation_is_stopped(&self) -> bool {
        self.propagation_stopped
    }

    #[inline(always)]
    pub fn redraw_is_requested(&self) -> bool {
        self.redraw_requested
    }
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum UiEvent {
    MouseMove(BlitzMouseButtonEvent),
    MouseUp(BlitzMouseButtonEvent),
    MouseDown(BlitzMouseButtonEvent),
    MouseLeave(BlitzMouseButtonEvent),
    KeyUp(BlitzKeyEvent),
    KeyDown(BlitzKeyEvent),
    Ime(BlitzImeEvent),
    // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
    Wheel(BlitzWheelEvent),
    // ORBITAL PATCH: multi-touch
    Touch(BlitzTouchEvent),
}
impl UiEvent {
    pub fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        // See: https://doc.rust-lang.org/stable/std/mem/fn.discriminant.html#accessing-the-numeric-value-of-the-discriminant
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

#[derive(Debug, Clone)]
pub struct DomEvent {
    pub target: usize,
    /// Which is true if the event bubbles up through the DOM tree.
    pub bubbles: bool,
    /// which is true if the event can be canceled.
    pub cancelable: bool,

    pub data: DomEventData,
    pub request_redraw: bool,
}

impl DomEvent {
    pub fn new(target: usize, data: DomEventData) -> Self {
        Self {
            target,
            bubbles: data.bubbles(),
            cancelable: data.cancelable(),
            data,
            request_redraw: false,
        }
    }

    /// Returns the name of the event ("click", "mouseover", "keypress", etc)
    pub fn name(&self) -> &'static str {
        self.data.name()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum DomEventKind {
    MouseMove,
    MouseDown,
    MouseUp,
    MouseLeave,
    Click,
    KeyPress,
    KeyDown,
    KeyUp,
    Input,
    Ime,
    DragStart,
    DragEnter,
    DragOver,
    DragLeave,
    Drop,
    DragEnd,
    // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
    Wheel,
    // ORBITAL PATCH: multi-touch
    TouchStart,
    TouchMove,
    TouchEnd,
    TouchCancel,
}
impl DomEventKind {
    pub fn discriminant(self) -> u8 {
        self as u8
    }
}
impl FromStr for DomEventKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s.trim_start_matches("on") {
            "mousemove" => Ok(Self::MouseMove),
            "mousedown" => Ok(Self::MouseDown),
            "mouseup" => Ok(Self::MouseUp),
            "mouseleave" => Ok(Self::MouseLeave),
            "click" => Ok(Self::Click),
            "keypress" => Ok(Self::KeyPress),
            "keydown" => Ok(Self::KeyDown),
            "keyup" => Ok(Self::KeyUp),
            "input" => Ok(Self::Input),
            "composition" => Ok(Self::Ime),
            "dragstart" => Ok(Self::DragStart),
            "dragenter" => Ok(Self::DragEnter),
            "dragover" => Ok(Self::DragOver),
            "dragleave" => Ok(Self::DragLeave),
            "drop" => Ok(Self::Drop),
            "dragend" => Ok(Self::DragEnd),
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            "wheel" => Ok(Self::Wheel),
            "touchstart" => Ok(Self::TouchStart),
            "touchmove" => Ok(Self::TouchMove),
            "touchend" => Ok(Self::TouchEnd),
            "touchcancel" => Ok(Self::TouchCancel),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum DomEventData {
    MouseMove(BlitzMouseButtonEvent),
    MouseDown(BlitzMouseButtonEvent),
    MouseUp(BlitzMouseButtonEvent),
    MouseLeave(BlitzMouseButtonEvent),
    Click(BlitzMouseButtonEvent),
    KeyPress(BlitzKeyEvent),
    KeyDown(BlitzKeyEvent),
    KeyUp(BlitzKeyEvent),
    Input(BlitzInputEvent),
    Ime(BlitzImeEvent),
    DragStart(BlitzDragEvent),
    DragEnter(BlitzDragEvent),
    DragOver(BlitzDragEvent),
    DragLeave(BlitzDragEvent),
    Drop(BlitzDragEvent),
    DragEnd(BlitzDragEvent),
    // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
    Wheel(BlitzWheelEvent),
    // ORBITAL PATCH: multi-touch
    TouchStart(BlitzTouchEvent),
    TouchMove(BlitzTouchEvent),
    TouchEnd(BlitzTouchEvent),
    TouchCancel(BlitzTouchEvent),
}
impl DomEventData {
    pub fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        // See: https://doc.rust-lang.org/stable/std/mem/fn.discriminant.html#accessing-the-numeric-value-of-the-discriminant
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

impl DomEventData {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MouseMove { .. } => "mousemove",
            Self::MouseDown { .. } => "mousedown",
            Self::MouseUp { .. } => "mouseup",
            Self::MouseLeave { .. } => "mouseleave",
            Self::Click { .. } => "click",
            Self::KeyPress { .. } => "keypress",
            Self::KeyDown { .. } => "keydown",
            Self::KeyUp { .. } => "keyup",
            Self::Input { .. } => "input",
            Self::Ime { .. } => "composition",
            Self::DragStart { .. } => "dragstart",
            Self::DragEnter { .. } => "dragenter",
            Self::DragOver { .. } => "dragover",
            Self::DragLeave { .. } => "dragleave",
            Self::Drop { .. } => "drop",
            Self::DragEnd { .. } => "dragend",
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            Self::Wheel { .. } => "wheel",
            Self::TouchStart { .. } => "touchstart",
            Self::TouchMove { .. } => "touchmove",
            Self::TouchEnd { .. } => "touchend",
            Self::TouchCancel { .. } => "touchcancel",
        }
    }

    pub fn kind(&self) -> DomEventKind {
        match self {
            Self::MouseMove { .. } => DomEventKind::MouseMove,
            Self::MouseDown { .. } => DomEventKind::MouseDown,
            Self::MouseUp { .. } => DomEventKind::MouseUp,
            Self::MouseLeave { .. } => DomEventKind::MouseLeave,
            Self::Click { .. } => DomEventKind::Click,
            Self::KeyPress { .. } => DomEventKind::KeyPress,
            Self::KeyDown { .. } => DomEventKind::KeyDown,
            Self::KeyUp { .. } => DomEventKind::KeyUp,
            Self::Input { .. } => DomEventKind::Input,
            Self::Ime { .. } => DomEventKind::Ime,
            Self::DragStart { .. } => DomEventKind::DragStart,
            Self::DragEnter { .. } => DomEventKind::DragEnter,
            Self::DragOver { .. } => DomEventKind::DragOver,
            Self::DragLeave { .. } => DomEventKind::DragLeave,
            Self::Drop { .. } => DomEventKind::Drop,
            Self::DragEnd { .. } => DomEventKind::DragEnd,
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            Self::Wheel { .. } => DomEventKind::Wheel,
            Self::TouchStart { .. } => DomEventKind::TouchStart,
            Self::TouchMove { .. } => DomEventKind::TouchMove,
            Self::TouchEnd { .. } => DomEventKind::TouchEnd,
            Self::TouchCancel { .. } => DomEventKind::TouchCancel,
        }
    }

    pub fn cancelable(&self) -> bool {
        match self {
            Self::MouseMove { .. } => true,
            Self::MouseDown { .. } => true,
            Self::MouseUp { .. } => true,
            Self::MouseLeave { .. } => false,
            Self::Click { .. } => true,
            Self::KeyDown { .. } => true,
            Self::KeyUp { .. } => true,
            Self::KeyPress { .. } => true,
            Self::Ime { .. } => true,
            Self::Input { .. } => false,
            Self::DragStart { .. }
            | Self::DragEnter { .. }
            | Self::DragOver { .. }
            | Self::Drop { .. } => true,
            Self::DragLeave { .. } | Self::DragEnd { .. } => false,
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            Self::Wheel { .. } => true,
            Self::TouchStart { .. } | Self::TouchMove { .. } | Self::TouchEnd { .. } => true,
            Self::TouchCancel { .. } => false,
        }
    }

    pub fn bubbles(&self) -> bool {
        match self {
            Self::MouseMove { .. } => true,
            Self::MouseDown { .. } => true,
            Self::MouseUp { .. } => true,
            Self::MouseLeave { .. } => false,
            Self::Click { .. } => true,
            Self::KeyDown { .. } => true,
            Self::KeyUp { .. } => true,
            Self::KeyPress { .. } => true,
            Self::Ime { .. } => true,
            Self::Input { .. } => true,
            Self::DragStart { .. }
            | Self::DragEnter { .. }
            | Self::DragOver { .. }
            | Self::DragLeave { .. }
            | Self::Drop { .. }
            | Self::DragEnd { .. } => true,
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            Self::Wheel { .. } => true,
            Self::TouchStart { .. }
            | Self::TouchMove { .. }
            | Self::TouchEnd { .. }
            | Self::TouchCancel { .. } => true,
        }
    }
}

#[derive(Debug)]
struct DragTransferState {
    data: HashMap<String, String>,
    effect_allowed: String,
    drop_effect: String,
}

impl Default for DragTransferState {
    fn default() -> Self {
        Self {
            data: HashMap::new(),
            effect_allowed: "all".into(),
            drop_effect: "none".into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BlitzDragTransfer(Arc<Mutex<DragTransferState>>);

impl BlitzDragTransfer {
    pub fn get_data(&self, mime: &str) -> Option<String> {
        self.0.lock().ok()?.data.get(mime).cloned()
    }

    pub fn set_data(&self, mime: &str, data: &str) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "drag transfer poisoned")?;
        state.data.insert(mime.to_string(), data.to_string());
        Ok(())
    }

    pub fn clear_data(&self, mime: Option<&str>) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "drag transfer poisoned")?;
        if let Some(mime) = mime {
            state.data.remove(mime);
        } else {
            state.data.clear();
        }
        Ok(())
    }

    pub fn effect_allowed(&self) -> String {
        self.0
            .lock()
            .map_or_else(|_| "none".into(), |state| state.effect_allowed.clone())
    }

    pub fn set_effect_allowed(&self, value: &str) {
        if let Ok(mut state) = self.0.lock() {
            state.effect_allowed = value.to_string();
        }
    }

    pub fn drop_effect(&self) -> String {
        self.0
            .lock()
            .map_or_else(|_| "none".into(), |state| state.drop_effect.clone())
    }

    pub fn set_drop_effect(&self, value: &str) {
        if let Ok(mut state) = self.0.lock() {
            state.drop_effect = value.to_string();
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlitzDragEvent {
    pub mouse: BlitzMouseButtonEvent,
    pub transfer: BlitzDragTransfer,
}

#[derive(Debug, Clone, Copy)]
pub struct HitResult {
    /// The node_id of the node identified as the hit target
    pub node_id: usize,
    /// The x coordinate of the hit within the hit target's border-box
    pub x: f32,
    /// The y coordinate of the hit within the hit target's border-box
    pub y: f32,
}

// ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
/// How a wheel event's deltas are measured, as in the DOM's `deltaMode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WheelDeltaMode {
    Pixel,
    Line,
}

// ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
/// A wheel event with DOM sign convention: positive `delta_y` scrolls content down (wheel toward the user).
#[derive(Clone, Debug)]
pub struct BlitzWheelEvent {
    pub x: f32,
    pub y: f32,
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_mode: WheelDeltaMode,
    pub buttons: MouseEventButtons,
    pub mods: Modifiers,
}

// ORBITAL PATCH: multi-touch
/// One finger on the screen: the platform's id for it and its position in the viewport.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlitzTouchPoint {
    pub id: u64,
    pub x: f32,
    pub y: f32,
}

// ORBITAL PATCH: multi-touch
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlitzTouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

// ORBITAL PATCH: multi-touch
/// A touch event as the DOM has it: the finger that changed and every finger still on the screen after it.
#[derive(Clone, Debug)]
pub struct BlitzTouchEvent {
    pub phase: BlitzTouchPhase,
    pub changed: BlitzTouchPoint,
    pub touches: Vec<BlitzTouchPoint>,
    pub mods: Modifiers,
}

#[derive(Clone, Debug)]
pub struct BlitzMouseButtonEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseEventButton,
    pub buttons: MouseEventButtons,
    pub mods: Modifiers,
}

bitflags! {
    /// The buttons property indicates which buttons are pressed on the mouse
    /// (or other input device) when a mouse event is triggered.
    ///
    /// [MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons)
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct MouseEventButtons: u8 {
        /// 0: No button or un-initialized
        const None = 0b0000_0000;
        /// 1: Primary button (usually the left button)
        const Primary = 0b0000_0001;
        /// 2: Secondary button (usually the right button)
        const Secondary = 0b0000_0010;
        /// 4: Auxiliary button (usually the mouse wheel button or middle button)
        const Auxiliary = 0b0000_0100;
        /// 8: 4th button (typically the "Browser Back" button)
        const Fourth = 0b0000_1000;
        /// 16: 5th button (typically the "Browser Forward" button)
        const Fifth = 0b0001_0000;
    }
}

impl Default for MouseEventButtons {
    fn default() -> Self {
        Self::None
    }
}

impl From<MouseEventButton> for MouseEventButtons {
    fn from(value: MouseEventButton) -> Self {
        match value {
            MouseEventButton::Main => Self::Primary,
            MouseEventButton::Auxiliary => Self::Auxiliary,
            MouseEventButton::Secondary => Self::Secondary,
            MouseEventButton::Fourth => Self::Fourth,
            MouseEventButton::Fifth => Self::Fifth,
        }
    }
}

/// The button property indicates which button was pressed
/// on the mouse to trigger the event.
///
/// [MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button)
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum MouseEventButton {
    /// Main button pressed, usually the left button or the un-initialized state
    #[default]
    Main = 0,
    /// Auxiliary button pressed, usually the wheel button or the middle button (if present)
    Auxiliary = 1,
    /// Secondary button pressed, usually the right button
    Secondary = 2,
    /// Fourth button, typically the Browser Back button
    Fourth = 3,
    /// Fifth button, typically the Browser Forward button
    Fifth = 4,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

impl KeyState {
    pub fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}

#[derive(Clone, Debug)]
pub struct BlitzKeyEvent {
    pub key: Key,
    pub code: Code,
    pub modifiers: Modifiers,
    pub location: Location,
    pub is_auto_repeating: bool,
    pub is_composing: bool,
    pub state: KeyState,
    pub text: Option<SmolStr>,
}

#[derive(Clone, Debug)]
pub struct BlitzInputEvent {
    pub value: String,
}

/// Copy of Winit IME event to avoid lower-level Blitz crates depending on winit
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlitzImeEvent {
    /// Notifies when the IME was enabled.
    ///
    /// After getting this event you could receive [`Preedit`][Self::Preedit] and
    /// [`Commit`][Self::Commit] events.
    Enabled,

    /// Notifies when a new composing text should be set at the cursor position.
    ///
    /// The value represents a pair of the preedit string and the cursor begin position and end
    /// position. When it's `None`, the cursor should be hidden. When `String` is an empty string
    /// this indicates that preedit was cleared.
    ///
    /// The cursor position is byte-wise indexed.
    Preedit(String, Option<(usize, usize)>),

    /// Notifies when text should be inserted into the editor widget.
    ///
    /// Right before this event winit will send empty [`Self::Preedit`] event.
    Commit(String),

    /// Complete Android GameActivity editor state with byte-indexed spans.
    TextState {
        text: String,
        selection: (usize, usize),
        compose_region: Option<(usize, usize)>,
    },

    /// Notifies when the IME was disabled.
    ///
    /// After receiving this event you won't get any more [`Preedit`][Self::Preedit] or
    /// [`Commit`][Self::Commit] events until the next [`Enabled`][Self::Enabled] event.
    Disabled,
}
