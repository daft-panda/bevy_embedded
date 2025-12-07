//! Input event handling for embedded windows

use bevy::ecs::resource::Resource;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;

/// Touch phase for touch input events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TouchPhase {
    /// Touch began
    Started = 0,
    /// Touch moved
    Moved = 1,
    /// Touch ended
    Ended = 2,
    /// Touch cancelled
    Cancelled = 3,
}

impl TouchPhase {
    /// Create a TouchPhase from a u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(TouchPhase::Started),
            1 => Some(TouchPhase::Moved),
            2 => Some(TouchPhase::Ended),
            3 => Some(TouchPhase::Cancelled),
            _ => None,
        }
    }
}

/// A touch input event from the host application
#[derive(Debug, Clone)]
pub struct EmbeddedTouchEvent {
    /// Touch phase
    pub phase: TouchPhase,
    /// Touch position in logical pixels
    pub position: Vec2,
    /// Unique identifier for this touch
    pub id: u64,
}

/// A keyboard input event from the host application
#[derive(Debug, Clone)]
pub struct EmbeddedKeyboardEvent {
    /// The key code
    pub key_code: KeyCode,
    /// The press state
    pub state: ButtonState,
}

/// A mouse button input event from the host application
#[derive(Debug, Clone)]
pub struct EmbeddedMouseButtonEvent {
    /// The mouse button
    pub button: MouseButton,
    /// The press state
    pub state: ButtonState,
}

/// A mouse motion event from the host application (delta movement)
#[derive(Debug, Clone)]
pub struct EmbeddedMouseMotionEvent {
    /// Delta movement in logical pixels
    pub delta: Vec2,
}

/// Resource that stores queued input events from the host application
#[derive(Resource, Default)]
pub struct EmbeddedInputEvents {
    /// Queued touch events
    pub touch_events: Vec<EmbeddedTouchEvent>,
    /// Queued keyboard events
    pub keyboard_events: Vec<EmbeddedKeyboardEvent>,
    /// Queued mouse button events
    pub mouse_button_events: Vec<EmbeddedMouseButtonEvent>,
    /// Queued mouse motion events
    pub mouse_motion_events: Vec<EmbeddedMouseMotionEvent>,
}

impl EmbeddedInputEvents {
    /// Adds a touch event to the queue
    pub fn add_touch_event(&mut self, event: EmbeddedTouchEvent) {
        self.touch_events.push(event);
    }

    /// Adds a keyboard event to the queue
    pub fn add_keyboard_event(&mut self, key_code: KeyCode, state: ButtonState) {
        self.keyboard_events
            .push(EmbeddedKeyboardEvent { key_code, state });
    }

    /// Adds a mouse button event to the queue
    pub fn add_mouse_button_event(&mut self, button: MouseButton, state: ButtonState) {
        self.mouse_button_events
            .push(EmbeddedMouseButtonEvent { button, state });
    }

    /// Adds a mouse motion event to the queue
    pub fn add_mouse_motion_event(&mut self, delta: Vec2) {
        self.mouse_motion_events
            .push(EmbeddedMouseMotionEvent { delta });
    }

    /// Clears all queued events (called after processing)
    pub fn clear(&mut self) {
        self.touch_events.clear();
        self.keyboard_events.clear();
        self.mouse_button_events.clear();
        self.mouse_motion_events.clear();
    }
}
