use floem_winit::{
    keyboard::{KeyCode, PhysicalKey},
    window::Theme,
};
use peniko::kurbo::{Point, Size};
use slotmap::Key;

use crate::{
    dropped_file::DroppedFileEvent,
    keyboard::KeyEvent,
    pointer::{PointerInputEvent, PointerMoveEvent, PointerWheelEvent},
    ViewId,
};

pub struct  EventResult(pub EventPropagation);

impl EventResult {

    pub fn event_continue() -> Self {
        Self(EventPropagation::Continue)
    }

    pub fn event_stop(view_id: ViewId, event: &Event) -> Self {
        use tracing::trace;
                match event {
                    Event::PointerDown(_) => {
                        trace!("PointerDown processed by {}", view_id.data().as_ffi());
                    }
                    Event::PointerUp(_) => {
                        trace!("PointerUp processed by {}", view_id.data().as_ffi());
                    }
                    Event::PointerMove(_) => {
                        trace!("PointerMove processed by {}", view_id.data().as_ffi());
                    }
                    Event::PointerWheel(_) => {
                        trace!("PointerWheel processed by {}", view_id.data().as_ffi());
                    }
                    Event::PointerLeave => {
                        trace!("PointerLeave processed by {}", view_id.data().as_ffi());
                    }
                    Event::DroppedFile(_) => {
                        trace!("DroppedFile processed by {}", view_id.data().as_ffi());
                    }
                    Event::KeyDown(_) => {
                        trace!("KeyDown processed by {}", view_id.data().as_ffi());
                    }
                    Event::KeyUp(_) => {
                        trace!("KeyUp processed by {}", view_id.data().as_ffi());
                    }
                    Event::ImeEnabled =>{
                        trace!("ImeEnabled processed by {}", view_id.data().as_ffi());
                    }
                    Event::ImeDisabled => {
                        trace!("ImeDisabled processed by {}", view_id.data().as_ffi());
                    }
                    Event::ImePreedit { .. } => {
                        trace!("ImePreedit processed by {}", view_id.data().as_ffi());
                    }
                    Event::ImeCommit(_) => {
                        trace!("ImeCommit processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowGotFocus => {
                        trace!("WindowGotFocus processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowLostFocus => {
                        trace!("WindowLostFocus processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowClosed =>{
                        trace!("WindowClosed processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowResized(_) => {
                        trace!("WindowResized processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowMoved(_) => {
                        trace!("WindowMoved processed by {}", view_id.data().as_ffi());
                    }
                    Event::WindowMaximizeChanged(_) => {
                        trace!("WindowMaximizeChanged processed by {}", view_id.data().as_ffi());
                    }
                    Event::ThemeChanged(_) => {
                        trace!("ThemeChanged processed by {}", view_id.data().as_ffi());
                    }
                    Event::FocusGained => {
                        trace!("FocusGained processed by {}", view_id.data().as_ffi());
                    }
                    Event::FocusLost => {
                        trace!("FocusLost processed by {}", view_id.data().as_ffi());
                    }
                }
        Self(EventPropagation::Stop)
    }
    pub fn is_processed(&self) -> bool {
        self.0.is_processed()
    }
}

/// Control whether an event will continue propagating or whether it should stop.
pub enum EventPropagation {
    /// Stop event propagation and mark the event as processed
    Stop,
    /// Let event propagation continue
    Continue,
}

impl EventPropagation {
    pub fn is_continue(&self) -> bool {
        matches!(self, EventPropagation::Continue)
    }

    pub fn is_stop(&self) -> bool {
        matches!(self, EventPropagation::Stop)
    }

    pub fn is_processed(&self) -> bool {
        matches!(self, EventPropagation::Stop)
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum EventListener {
    /// Receives [`Event::KeyDown`]
    KeyDown,
    /// Receives [`Event::KeyUp`]
    KeyUp,
    /// Receives [`Event::PointerUp`] or [`Event::KeyDown`]
    /// `KeyDown` occurs when using enter on a focused element, such as a button.
    Click,
    /// Receives [`Event::PointerUp`]
    DoubleClick,
    /// Receives [`Event::PointerUp`]
    SecondaryClick,
    /// Receives [`Event::PointerMove`]
    DragStart,
    /// Receives [`Event::PointerUp`]
    DragEnd,
    /// Receives [`Event::PointerMove`]
    DragOver,
    /// Receives [`Event::PointerMove`]
    DragEnter,
    /// Receives [`Event::PointerMove`]
    DragLeave,
    /// Receives [`Event::PointerUp`]
    Drop,
    /// Receives [`Event::PointerDown`]
    PointerDown,
    /// Receives [`Event::PointerMove`]
    PointerMove,
    /// Receives [`Event::PointerUp`]
    PointerUp,
    /// Receives [`Event::PointerMove`]
    PointerEnter,
    /// Receives [`Event::PointerLeave`]
    PointerLeave,
    /// Receives [`Event::ImeEnabled`]
    ImeEnabled,
    /// Receives [`Event::ImeDisabled`]
    ImeDisabled,
    /// Receives [`Event::ImePreedit`]
    ImePreedit,
    /// Receives [`Event::ImeCommit`]
    ImeCommit,
    /// Receives [`Event::PointerWheel`]
    PointerWheel,
    /// Receives [`Event::FocusGained`]
    FocusGained,
    /// Receives [`Event::FocusLost`]
    FocusLost,
    /// Receives [`Event::ThemeChanged`]
    ThemeChanged,
    /// Receives [`Event::WindowClosed`]
    WindowClosed,
    /// Receives [`Event::WindowResized`]
    WindowResized,
    /// Receives [`Event::WindowMoved`]
    WindowMoved,
    /// Receives [`Event::WindowGotFocus`]
    WindowGotFocus,
    /// Receives [`Event::WindowLostFocus`]
    WindowLostFocus,
    /// Receives [`Event::WindowMaximizeChanged`]
    WindowMaximizeChanged,
    /// Receives [`Event::DroppedFile`]
    DroppedFile,
}

#[derive(Debug, Clone)]
pub enum Event {
    PointerDown(PointerInputEvent),
    PointerUp(PointerInputEvent),
    PointerMove(PointerMoveEvent),
    PointerWheel(PointerWheelEvent),
    PointerLeave,
    DroppedFile(DroppedFileEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    ImeEnabled,
    ImeDisabled,
    ImePreedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    ImeCommit(String),
    WindowGotFocus,
    WindowLostFocus,
    WindowClosed,
    WindowResized(Size),
    WindowMoved(Point),
    WindowMaximizeChanged(bool),
    ThemeChanged(Theme),
    FocusGained,
    FocusLost,
}

impl Event {
    pub fn needs_focus(&self) -> bool {
        match self {
            Event::PointerDown(_)
            | Event::PointerUp(_)
            | Event::PointerMove(_)
            | Event::PointerWheel(_)
            | Event::PointerLeave
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ImeCommit(_)
            | Event::ThemeChanged(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowMaximizeChanged(_)
            | Event::WindowGotFocus
            | Event::WindowLostFocus
            | Event::DroppedFile(_) => false,
            Event::KeyDown(_) | Event::KeyUp(_) => true,
        }
    }

    pub(crate) fn is_pointer(&self) -> bool {
        match self {
            Event::PointerDown(_)
            | Event::PointerUp(_)
            | Event::PointerMove(_)
            | Event::PointerWheel(_)
            | Event::PointerLeave => true,
            Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ImeCommit(_)
            | Event::ThemeChanged(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowMaximizeChanged(_)
            | Event::WindowGotFocus
            | Event::WindowLostFocus
            | Event::DroppedFile(_) => false,
        }
    }

    /// Enter, numpad enter and space cause a view to be activated with the keyboard
    pub(crate) fn is_keyboard_trigger(&self) -> bool {
        match self {
            Event::KeyDown(key) | Event::KeyUp(key) => {
                matches!(
                    key.key.physical_key,
                    PhysicalKey::Code(KeyCode::NumpadEnter)
                        | PhysicalKey::Code(KeyCode::Enter)
                        | PhysicalKey::Code(KeyCode::Space),
                )
            }
            _ => false,
        }
    }

    pub fn allow_disabled(&self) -> bool {
        match self {
            Event::PointerDown(_)
            | Event::PointerUp(_)
            | Event::PointerWheel(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ImeCommit(_)
            | Event::KeyDown(_)
            | Event::KeyUp(_) => false,
            Event::PointerLeave
            | Event::PointerMove(_)
            | Event::ThemeChanged(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowGotFocus
            | Event::WindowMaximizeChanged(_)
            | Event::WindowLostFocus
            | Event::DroppedFile(_) => true,
        }
    }

    pub fn point(&self) -> Option<Point> {
        match self {
            Event::PointerDown(pointer_event) | Event::PointerUp(pointer_event) => {
                Some(pointer_event.pos)
            }
            Event::PointerMove(pointer_event) => Some(pointer_event.pos),
            Event::PointerWheel(pointer_event) => Some(pointer_event.pos),
            Event::DroppedFile(event) => Some(event.pos),
            Event::PointerLeave
            | Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ThemeChanged(_)
            | Event::ImeCommit(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowMaximizeChanged(_)
            | Event::WindowGotFocus
            | Event::WindowLostFocus => None,
        }
    }

    pub fn scale(mut self, scale: f64) -> Event {
        match &mut self {
            Event::PointerDown(pointer_event) | Event::PointerUp(pointer_event) => {
                pointer_event.pos.x /= scale;
                pointer_event.pos.y /= scale;
            }
            Event::PointerMove(pointer_event) => {
                pointer_event.pos.x /= scale;
                pointer_event.pos.y /= scale;
            }
            Event::PointerWheel(pointer_event) => {
                pointer_event.pos.x /= scale;
                pointer_event.pos.y /= scale;
            }
            Event::DroppedFile(event) => {
                event.pos.x /= scale;
                event.pos.y /= scale;
            }
            Event::PointerLeave
            | Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ThemeChanged(_)
            | Event::ImeCommit(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowMaximizeChanged(_)
            | Event::WindowGotFocus
            | Event::WindowLostFocus => {}
        }
        self
    }

    pub fn offset(mut self, offset: (f64, f64)) -> Event {
        match &mut self {
            Event::PointerDown(pointer_event) | Event::PointerUp(pointer_event) => {
                pointer_event.pos -= offset;
            }
            Event::PointerMove(pointer_event) => {
                pointer_event.pos -= offset;
            }
            Event::PointerWheel(pointer_event) => {
                pointer_event.pos -= offset;
            }
            Event::DroppedFile(event) => {
                event.pos -= offset;
            }
            Event::PointerLeave
            | Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::FocusGained
            | Event::FocusLost
            | Event::ImeEnabled
            | Event::ImeDisabled
            | Event::ImePreedit { .. }
            | Event::ThemeChanged(_)
            | Event::ImeCommit(_)
            | Event::WindowClosed
            | Event::WindowResized(_)
            | Event::WindowMoved(_)
            | Event::WindowMaximizeChanged(_)
            | Event::WindowGotFocus
            | Event::WindowLostFocus => {}
        }
        self
    }

    pub fn listener(&self) -> Option<EventListener> {
        match self {
            Event::PointerDown(_) => Some(EventListener::PointerDown),
            Event::PointerUp(_) => Some(EventListener::PointerUp),
            Event::PointerMove(_) => Some(EventListener::PointerMove),
            Event::PointerWheel(_) => Some(EventListener::PointerWheel),
            Event::PointerLeave => Some(EventListener::PointerLeave),
            Event::KeyDown(_) => Some(EventListener::KeyDown),
            Event::KeyUp(_) => Some(EventListener::KeyUp),
            Event::ImeEnabled => Some(EventListener::ImeEnabled),
            Event::ImeDisabled => Some(EventListener::ImeDisabled),
            Event::ImePreedit { .. } => Some(EventListener::ImePreedit),
            Event::ImeCommit(_) => Some(EventListener::ImeCommit),
            Event::WindowClosed => Some(EventListener::WindowClosed),
            Event::WindowResized(_) => Some(EventListener::WindowResized),
            Event::WindowMoved(_) => Some(EventListener::WindowMoved),
            Event::WindowMaximizeChanged(_) => Some(EventListener::WindowMaximizeChanged),
            Event::WindowGotFocus => Some(EventListener::WindowGotFocus),
            Event::WindowLostFocus => Some(EventListener::WindowLostFocus),
            Event::FocusLost => Some(EventListener::FocusLost),
            Event::FocusGained => Some(EventListener::FocusGained),
            Event::ThemeChanged(_) => Some(EventListener::ThemeChanged),
            Event::DroppedFile(_) => Some(EventListener::DroppedFile),
        }
    }
}
