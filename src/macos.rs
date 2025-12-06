//! macOS-specific embedded integration
//!
//! This module provides the ability to embed Bevy into a macOS application
//! by providing an NSView that Bevy can render into.

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unsafe_attr_outside_unsafe)]
#![allow(unsafe_code)]

use bevy::app::App;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use bevy::window::{
    PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, Window, WindowResolution,
    WindowWrapper,
};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use crate::config::take_config;
use crate::{EmbeddedInputEvents, EmbeddedTouchEvent, HostChannel, TouchPhase};

/// Wrapper for the NSView that implements the required traits for raw-window-handle
struct NSViewWrapper {
    window_handle: AppKitWindowHandle,
    display_handle: AppKitDisplayHandle,
}

unsafe impl Send for NSViewWrapper {}
unsafe impl Sync for NSViewWrapper {}

impl HasWindowHandle for NSViewWrapper {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::WindowHandle::borrow_raw(
                RawWindowHandle::AppKit(self.window_handle),
            ))
        }
    }
}

impl HasDisplayHandle for NSViewWrapper {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::DisplayHandle::borrow_raw(
                RawDisplayHandle::AppKit(self.display_handle),
            ))
        }
    }
}

/// Called during app creation to create the window from the stored config
pub fn create_window_from_host(app: &mut App) {
    let config = take_config();

    let Some(config) = config else {
        log::error!("No config available. Call bevy_embedded_set_config first.");
        return;
    };

    if config.view.is_null() {
        log::error!("Host provided a null view pointer");
        return;
    }

    log::info!(
        "Creating embedded macOS window: {}x{} @ {}x scale",
        config.width,
        config.height,
        config.scale_factor
    );

    // Create the view wrapper for raw-window-handle
    let view_wrapper = NSViewWrapper {
        window_handle: unsafe {
            AppKitWindowHandle::new(NonNull::new_unchecked(config.view as *mut _))
        },
        display_handle: AppKitDisplayHandle::new(),
    };

    // Create WindowWrapper and RawHandleWrapper
    let window_wrapper = WindowWrapper::new(view_wrapper);
    let handle_wrapper =
        RawHandleWrapper::new(&window_wrapper).expect("Failed to create RawHandleWrapper");

    let handle_holder = RawHandleWrapperHolder(Arc::new(Mutex::new(Some(handle_wrapper.clone()))));

    // Create the Window entity with the native surface
    let window = Window {
        resolution: WindowResolution::new(config.width, config.height)
            .with_scale_factor_override(config.scale_factor),
        ..Default::default()
    };

    app.world_mut()
        .spawn((window, handle_wrapper, handle_holder, PrimaryWindow));

    log::info!("Embedded macOS window created successfully");
}

/// Handle a mouse button event from macOS
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
/// - `button`: 0 = Left, 1 = Right, 2 = Middle
/// - `pressed`: true if pressed, false if released
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_macos_mouse_button(
    app: *mut c_void,
    button: u8,
    pressed: bool,
    x: f32,
    y: f32,
) {
    if app.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);

    // Convert to touch event for compatibility (touch ID based on button)
    // This is a simplification - a full implementation would use MouseButtonInput
    let phase = if pressed {
        TouchPhase::Started
    } else {
        TouchPhase::Ended
    };

    let mut input_events = app.world_mut().resource_mut::<EmbeddedInputEvents>();
    input_events.add_touch_event(EmbeddedTouchEvent {
        phase,
        position: Vec2::new(x, y),
        id: button as u64,
    });
}

/// Handle a mouse move event from macOS
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_macos_mouse_moved(app: *mut c_void, x: f32, y: f32) {
    if app.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);

    // Convert to touch move for compatibility
    let mut input_events = app.world_mut().resource_mut::<EmbeddedInputEvents>();
    input_events.add_touch_event(EmbeddedTouchEvent {
        phase: TouchPhase::Moved,
        position: Vec2::new(x, y),
        id: 0, // Primary mouse button
    });
}

/// Handle a resize event from macOS
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_macos_resize(
    app: *mut c_void,
    width: u32,
    height: u32,
    scale_factor: f32,
) {
    if app.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);

    // Find the primary window and update its resolution
    let mut query = app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut window) = query.single_mut(app.world_mut()) {
        window.resolution.set_physical_resolution(width, height);
        window.resolution.set_scale_factor(scale_factor);
        log::debug!(
            "macOS window resized to {}x{} @ {}x",
            width,
            height,
            scale_factor
        );
    }
}

/// Send a binary message to Bevy from the host
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
/// - `data` must be a valid pointer to `len` bytes
/// - The data will be copied, so the caller retains ownership
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_macos_send_message(
    app: *mut c_void,
    data: *const u8,
    len: usize,
) {
    if app.is_null() || data.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);
    let slice = std::slice::from_raw_parts(data, len);
    let message = slice.to_vec();

    if let Some(channel) = app.world().get_resource::<HostChannel>() {
        channel.send(message);
    } else {
        log::warn!("HostChannel resource not available");
    }
}

/// Receive a binary message from Bevy (non-blocking poll)
///
/// Returns the number of bytes read, or 0 if no message is available.
/// The buffer must be at least `buffer_len` bytes.
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
/// - `buffer` must be a valid pointer to at least `buffer_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_macos_receive_message(
    app: *mut c_void,
    buffer: *mut u8,
    buffer_len: usize,
) -> usize {
    if app.is_null() || buffer.is_null() || buffer_len == 0 {
        return 0;
    }

    let app = &mut *(app as *mut App);

    if let Some(channel) = app.world().get_resource::<HostChannel>() {
        if let Some(message) = channel.receive() {
            let copy_len = message.len().min(buffer_len);
            std::ptr::copy_nonoverlapping(message.as_ptr(), buffer, copy_len);
            return copy_len;
        }
    }

    0
}
