//! iOS-specific embedded integration

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
    HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    UiKitDisplayHandle, UiKitWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use crate::{EmbeddedInputEvents, EmbeddedTouchEvent, HostChannel, TouchPhase};

/// Wrapper for the UIView that implements the required traits
struct MetalViewWrapper {
    window_handle: UiKitWindowHandle,
    display_handle: UiKitDisplayHandle,
}

unsafe impl Send for MetalViewWrapper {}
unsafe impl Sync for MetalViewWrapper {}

impl HasWindowHandle for MetalViewWrapper {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::WindowHandle::borrow_raw(
                RawWindowHandle::UiKit(self.window_handle),
            ))
        }
    }
}

impl HasDisplayHandle for MetalViewWrapper {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::DisplayHandle::borrow_raw(
                RawDisplayHandle::UiKit(self.display_handle),
            ))
        }
    }
}

/// Surface info returned from the host app
#[repr(C)]
pub struct EmbeddedSurfaceInfo {
    pub ui_view: *const c_void,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

/// Called by EmbeddedPlugin during finish() to create the window
/// This requests the native surface from the host application
pub fn create_window_from_host(app: &mut App) {
    // Call into Swift to get the surface info
    unsafe extern "C" {
        fn bevy_embedded_get_surface(out: *mut EmbeddedSurfaceInfo);
    }

    let mut surface_info = EmbeddedSurfaceInfo {
        ui_view: std::ptr::null(),
        width: 0,
        height: 0,
        scale_factor: 1.0,
    };

    unsafe { bevy_embedded_get_surface(&mut surface_info) };

    if surface_info.ui_view.is_null() {
        log::error!("Host did not provide a valid surface");
        return;
    }

    log::info!(
        "Creating embedded window: {}x{} @ {}x scale",
        surface_info.width,
        surface_info.height,
        surface_info.scale_factor
    );

    // Create the view wrapper for raw-window-handle
    let view_wrapper = MetalViewWrapper {
        window_handle: unsafe {
            UiKitWindowHandle::new(NonNull::new_unchecked(surface_info.ui_view as *mut _))
        },
        display_handle: UiKitDisplayHandle::new(),
    };

    // Create WindowWrapper and RawHandleWrapper
    let window_wrapper = WindowWrapper::new(view_wrapper);
    let handle_wrapper =
        RawHandleWrapper::new(&window_wrapper).expect("Failed to create RawHandleWrapper");

    let handle_holder = RawHandleWrapperHolder(Arc::new(Mutex::new(Some(handle_wrapper.clone()))));

    // Create the Window entity with the native surface
    let window = Window {
        resolution: WindowResolution::new(surface_info.width, surface_info.height)
            .with_scale_factor_override(surface_info.scale_factor),
        ..Default::default()
    };

    app.world_mut()
        .spawn((window, handle_wrapper, handle_holder, PrimaryWindow));

    log::info!("Embedded window created successfully");
}

/// Handle a touch event from iOS
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
/// - `phase`: 0 = Started, 1 = Moved, 2 = Ended, 3 = Cancelled
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_ios_touch_event(
    app: *mut c_void,
    phase: u8,
    x: f32,
    y: f32,
    id: u64,
) {
    if app.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);

    if let Some(touch_phase) = TouchPhase::from_u8(phase) {
        let mut input_events = app.world_mut().resource_mut::<EmbeddedInputEvents>();
        input_events.add_touch_event(EmbeddedTouchEvent {
            phase: touch_phase,
            position: Vec2::new(x, y),
            id,
        });
    }
}

/// Handle a resize event from iOS
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_ios_resize(
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
pub unsafe extern "C" fn bevy_embedded_ios_send_message(
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

    // Check if the resource exists before accessing it
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
pub unsafe extern "C" fn bevy_embedded_ios_receive_message(
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
