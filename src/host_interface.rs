//! Host interface trait for embedded Bevy applications
//!
//! Bevy uses this trait to request resources from the host application.
//!
//! - For mobile (iOS/Android): Default implementation uses extern C symbols
//! - For desktop: Implement the trait directly, no extern symbols needed

use bevy::app::App;
use bevy::ecs::query::With;
use bevy::window::{PrimaryWindow, Window};
use std::ffi::c_void;

use crate::HostChannel;

/// Surface information provided by the host
pub struct SurfaceInfo {
    /// Pointer to the native view/surface (NSView, UIView, ANativeWindow, wl_surface, etc.)
    pub view: *const c_void,
    /// Width in physical pixels
    pub width: u32,
    /// Height in physical pixels
    pub height: u32,
    /// Scale factor for retina/high-DPI displays
    pub scale_factor: f32,
    /// Linux/Wayland: pointer to wl_display
    #[cfg(target_os = "linux")]
    pub wayland_display: *const c_void,
    /// Linux/Wayland: X position relative to parent surface
    #[cfg(target_os = "linux")]
    pub x: i32,
    /// Linux/Wayland: Y position relative to parent surface
    #[cfg(target_os = "linux")]
    pub y: i32,
}

// Safety: The view pointer is only accessed from the main thread where
// the Bevy app is created and updated. The HostInterface trait requires
// Send+Sync so the host can store the interface, but actual surface access
// happens on the main thread.
unsafe impl Send for SurfaceInfo {}
unsafe impl Sync for SurfaceInfo {}

/// Trait for Bevy to request resources from the host application
pub trait HostInterface: Send + Sync {
    /// Get the native surface/view from the host
    fn get_surface(&self) -> Option<SurfaceInfo>;
}

/// Default implementation using external C symbols
///
/// This is used on iOS where the host (Swift) provides a `bevy_embedded_get_surface` function.
#[cfg(target_os = "ios")]
pub struct ExternHostInterface;

#[cfg(target_os = "ios")]
impl ExternHostInterface {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "ios")]
impl Default for ExternHostInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "ios")]
impl HostInterface for ExternHostInterface {
    fn get_surface(&self) -> Option<SurfaceInfo> {
        #[repr(C)]
        struct CSurfaceInfo {
            view: *const c_void,
            width: u32,
            height: u32,
            scale_factor: f32,
        }

        extern "C" {
            fn bevy_embedded_get_surface(out: *mut CSurfaceInfo);
        }

        let mut info = CSurfaceInfo {
            view: std::ptr::null(),
            width: 0,
            height: 0,
            scale_factor: 1.0,
        };

        unsafe { bevy_embedded_get_surface(&mut info) };

        if info.view.is_null() {
            None
        } else {
            Some(SurfaceInfo {
                view: info.view,
                width: info.width,
                height: info.height,
                scale_factor: info.scale_factor,
            })
        }
    }
}

/// macOS uses config-based initialization, not a callback
#[cfg(target_os = "macos")]
pub struct ExternHostInterface;

#[cfg(target_os = "macos")]
impl ExternHostInterface {
    /// Create a new extern host interface
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
impl Default for ExternHostInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
impl HostInterface for ExternHostInterface {
    fn get_surface(&self) -> Option<SurfaceInfo> {
        // macOS uses set_config before app creation, surface comes from there
        crate::config::take_config().map(|config| SurfaceInfo {
            view: config.view,
            width: config.width,
            height: config.height,
            scale_factor: config.scale_factor,
        })
    }
}

// ============================================================================
// Common helpers for host-to-Bevy operations (used by platform FFI functions)
// ============================================================================

/// Resize the primary window
pub fn resize_window(app: &mut App, width: u32, height: u32, scale_factor: f32) {
    let mut query = app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut window) = query.single_mut(app.world_mut()) {
        window.resolution.set_physical_resolution(width, height);
        window.resolution.set_scale_factor(scale_factor);
        log::debug!("Window resized to {}x{} @ {}x", width, height, scale_factor);
    }
}

/// Send a binary message to Bevy
pub fn send_message(app: &App, data: Vec<u8>) {
    if let Some(channel) = app.world().get_resource::<HostChannel>() {
        channel.send(data);
    } else {
        log::warn!("HostChannel resource not available");
    }
}

/// Receive a binary message from Bevy (non-blocking)
pub fn receive_message(app: &App) -> Option<Vec<u8>> {
    app.world()
        .get_resource::<HostChannel>()
        .and_then(|channel| channel.receive())
}

// ============================================================================
// Input forwarding helpers
// ============================================================================

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;

use crate::EmbeddedInputEvents;

/// Send a keyboard event to Bevy
pub fn send_keyboard_event(app: &mut App, key_code: KeyCode, state: ButtonState) {
    if let Some(mut input_events) = app.world_mut().get_resource_mut::<EmbeddedInputEvents>() {
        input_events.add_keyboard_event(key_code, state);
    }
}

/// Send a mouse button event to Bevy
pub fn send_mouse_button_event(app: &mut App, button: MouseButton, state: ButtonState) {
    if let Some(mut input_events) = app.world_mut().get_resource_mut::<EmbeddedInputEvents>() {
        input_events.add_mouse_button_event(button, state);
    }
}

/// Send a mouse motion event to Bevy
pub fn send_mouse_motion_event(app: &mut App, delta: Vec2) {
    if let Some(mut input_events) = app.world_mut().get_resource_mut::<EmbeddedInputEvents>() {
        input_events.add_mouse_motion_event(delta);
    }
}
