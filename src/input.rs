//! Input event handling for embedded windows

use bevy::{ecs::resource::Resource, math::Vec2};

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

/// Resource that stores queued input events from the host application
#[derive(Resource, Default)]
pub struct EmbeddedInputEvents {
    /// Queued touch events
    pub touch_events: Vec<EmbeddedTouchEvent>,
}

impl EmbeddedInputEvents {
    /// Adds a touch event to the queue
    pub fn add_touch_event(&mut self, event: EmbeddedTouchEvent) {
        self.touch_events.push(event);
    }

    /// Clears all queued events (called after processing)
    pub fn clear(&mut self) {
        self.touch_events.clear();
    }
}
