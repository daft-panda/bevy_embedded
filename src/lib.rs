//! Embedded widget support for Bevy Engine
//!
//! This crate provides the ability to embed Bevy as a widget within native applications
//! on iOS and Android platforms. Instead of using winit to manage windows and input,
//! the host application provides a surface (CAMetalLayer on iOS, SurfaceView on Android)
//! and forwards input events to Bevy.
//!
//! # Architecture
//!
//! - **EmbeddedPlugin**: Replaces WinitPlugin for embedded mode
//! - Uses existing `Window` component from bevy_window
//! - Provides FFI for injecting window handles and input events
//! - **BinaryChannel**: Bidirectional communication between Bevy and the host

#![warn(missing_docs)]

mod app_trait;
mod channel;
mod input;
mod plugin;

#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_os = "android")]
pub mod android;

pub use app_trait::*;
pub use channel::*;
pub use input::*;
pub use plugin::*;

#[cfg(target_os = "ios")]
pub use ios::*;

#[cfg(target_os = "android")]
pub use android::*;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{app_trait::*, channel::*, input::*, plugin::EmbeddedPlugin};

    #[cfg(target_os = "ios")]
    pub use crate::ios::*;

    #[cfg(target_os = "android")]
    pub use crate::android::*;
}
