//! Embedded widget support for Bevy Engine
//!
//! This crate provides the ability to embed Bevy as a widget within native applications
//! on iOS, macOS, Android and WASM platforms. Instead of using winit to manage windows and input,
//! the host application provides a surface (CAMetalLayer on iOS, NSView on macOS,
//! SurfaceView on Android, Canvas on WASM) and forwards input events to Bevy.
//!
//! # Architecture
//!
//! - **EmbeddedPlugin**: Replaces WinitPlugin for embedded mode
//! - Uses existing `Window` component from bevy_window
//! - Provides FFI for injecting window handles and input events
//! - **BinaryChannel**: Bidirectional communication between Bevy and the host

#![warn(missing_docs)]

/// Re-export bevy so consumers don't need to add it as a separate dependency
pub use bevy;

mod app_trait;
mod channel;
mod config;
pub mod host_interface;
mod input;
mod plugin;

#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use app_trait::*;
pub use channel::*;
pub use config::*;
pub use input::*;
pub use plugin::*;

#[cfg(target_os = "ios")]
pub use ios::*;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "android")]
pub use android::*;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{app_trait::*, channel::*, input::*, plugin::EmbeddedPlugin};

    #[cfg(target_os = "ios")]
    pub use crate::ios::*;

    #[cfg(target_os = "macos")]
    pub use crate::macos::*;

    #[cfg(target_os = "android")]
    pub use crate::android::*;

    #[cfg(target_os = "linux")]
    pub use crate::linux::*;

    #[cfg(target_arch = "wasm32")]
    pub use crate::wasm::*;
}
