//! iOS-specific embedded integration

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unsafe_attr_outside_unsafe)]
#![allow(unsafe_code)]

use bevy::app::App;
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

use crate::host_interface::{HostInterface, SurfaceInfo};
use crate::{EmbeddedInputEvents, EmbeddedTouchEvent, TouchPhase};

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

/// Called during app creation to create the window from the host interface
pub fn create_window_from_host_with<H: HostInterface>(app: &mut App, host: &H) {
    let Some(surface) = host.get_surface() else {
        log::error!("Host did not provide a surface");
        return;
    };

    create_window_from_surface(app, surface);
}

/// Called during app creation using the default ExternHostInterface
pub fn create_window_from_host(app: &mut App) {
    use crate::host_interface::ExternHostInterface;
    create_window_from_host_with(app, &ExternHostInterface::new());
}

/// Create window from surface info
pub fn create_window_from_surface(app: &mut App, surface: SurfaceInfo) {
    if surface.view.is_null() {
        log::error!("Host provided a null view pointer");
        return;
    }

    log::info!(
        "Creating embedded iOS window: {}x{} @ {}x scale",
        surface.width,
        surface.height,
        surface.scale_factor
    );

    // Create the view wrapper for raw-window-handle
    let view_wrapper = MetalViewWrapper {
        window_handle: unsafe {
            UiKitWindowHandle::new(NonNull::new_unchecked(surface.view as *mut _))
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
        resolution: WindowResolution::new(surface.width, surface.height)
            .with_scale_factor_override(surface.scale_factor),
        ..Default::default()
    };

    app.world_mut()
        .spawn((window, handle_wrapper, handle_holder, PrimaryWindow));

    log::info!("Embedded iOS window created successfully");
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
    crate::host_interface::resize_window(app, width, height, scale_factor);
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
    let app = &*(app as *mut App);
    let message = std::slice::from_raw_parts(data, len).to_vec();
    crate::host_interface::send_message(app, message);
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
    let app = &*(app as *mut App);
    if let Some(message) = crate::host_interface::receive_message(app) {
        let copy_len = message.len().min(buffer_len);
        std::ptr::copy_nonoverlapping(message.as_ptr(), buffer, copy_len);
        copy_len
    } else {
        0
    }
}
