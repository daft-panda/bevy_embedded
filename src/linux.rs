//! Linux/Wayland-specific embedded integration
//!
//! This module provides the ability to embed Bevy into a Linux application
//! by providing a Wayland surface that Bevy can render into.
//!
//! The key challenge on Wayland is that we cannot render directly to a surface
//! that is already managed by another toolkit (like GTK). Instead, we create
//! a subsurface that is parented to the host's surface, and Bevy renders to that.

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
    WaylandDisplayHandle, WaylandWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use wayland_backend::client::Backend;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_compositor, wl_region, wl_registry, wl_subcompositor, wl_subsurface, wl_surface,
    },
};

use crate::host_interface::{HostInterface, SurfaceInfo};
use crate::{EmbeddedInputEvents, EmbeddedTouchEvent, TouchPhase};

/// Holds the Wayland objects we need to keep alive and allows repositioning
pub struct WaylandSubsurface {
    /// The surface we created for Bevy to render to
    _surface: wl_surface::WlSurface,
    /// The subsurface relationship (kept for repositioning)
    subsurface: wl_subsurface::WlSubsurface,
    /// Keep the connection alive
    connection: Connection,
}

impl WaylandSubsurface {
    /// Update the subsurface position relative to the parent surface
    pub fn set_position(&self, x: i32, y: i32) {
        self.subsurface.set_position(x, y);
        if let Err(e) = self.connection.flush() {
            log::warn!("Failed to flush connection after set_position: {}", e);
        }
    }
}

impl Drop for WaylandSubsurface {
    fn drop(&mut self) {
        log::info!("Destroying Wayland subsurface and surface");
        // Destroy the subsurface relationship first
        self.subsurface.destroy();
        // Then destroy the surface
        self._surface.destroy();
        if let Err(e) = self.connection.flush() {
            log::warn!("Failed to flush connection after destroy: {}", e);
        }
    }
}

unsafe impl Send for WaylandSubsurface {}
unsafe impl Sync for WaylandSubsurface {}

/// Wrapper for the Wayland surface that implements the required traits for raw-window-handle
struct WaylandSurfaceWrapper {
    window_handle: WaylandWindowHandle,
    display_handle: WaylandDisplayHandle,
    /// Keep the subsurface alive
    _subsurface: Option<Arc<WaylandSubsurface>>,
}

unsafe impl Send for WaylandSurfaceWrapper {}
unsafe impl Sync for WaylandSurfaceWrapper {}

impl HasWindowHandle for WaylandSurfaceWrapper {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::WindowHandle::borrow_raw(
                RawWindowHandle::Wayland(self.window_handle),
            ))
        }
    }
}

impl HasDisplayHandle for WaylandSurfaceWrapper {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::DisplayHandle::borrow_raw(
                RawDisplayHandle::Wayland(self.display_handle),
            ))
        }
    }
}

/// State for the Wayland registry (fields unused but required for Dispatch impl)
#[allow(dead_code)]
struct RegistryState {
    compositor: Option<wl_compositor::WlCompositor>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_subcompositor::WlSubcompositor, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_subcompositor::WlSubcompositor,
        _event: wl_subcompositor::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_subsurface::WlSubsurface, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_subsurface::WlSubsurface,
        _event: wl_subsurface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_region::WlRegion, ()> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_region::WlRegion,
        _event: wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

/// Called during app creation to create the window from the host interface.
/// Returns the WaylandSubsurface handle which can be used for repositioning.
pub fn create_window_from_host_with<H: HostInterface>(
    app: &mut App,
    host: &H,
) -> Option<Arc<WaylandSubsurface>> {
    let Some(surface) = host.get_surface() else {
        log::error!("Host did not provide a surface");
        return None;
    };

    create_window_from_surface(app, surface)
}

/// Create window from surface info.
/// This will create a subsurface parented to the provided surface.
/// Returns the WaylandSubsurface handle which can be used for repositioning.
pub fn create_window_from_surface(
    app: &mut App,
    surface: SurfaceInfo,
) -> Option<Arc<WaylandSubsurface>> {
    if surface.view.is_null() {
        log::error!("Host provided a null surface pointer");
        return None;
    }

    if surface.wayland_display.is_null() {
        log::error!("Host provided a null display pointer");
        return None;
    }

    log::info!(
        "Creating embedded Linux/Wayland window: {}x{} @ {}x scale, parent_surface={:p}, display={:p}",
        surface.width,
        surface.height,
        surface.scale_factor,
        surface.view,
        surface.wayland_display
    );

    // Try to create a subsurface; fall back to using the parent surface directly if it fails
    let (surface_wrapper, subsurface_handle) = match create_subsurface(&surface) {
        Ok((bevy_surface_ptr, subsurface)) => {
            log::info!(
                "Created Wayland subsurface for Bevy: {:p}",
                bevy_surface_ptr
            );
            let subsurface_arc = Arc::new(subsurface);
            (
                WaylandSurfaceWrapper {
                    window_handle: unsafe {
                        WaylandWindowHandle::new(NonNull::new_unchecked(bevy_surface_ptr as *mut _))
                    },
                    display_handle: unsafe {
                        WaylandDisplayHandle::new(NonNull::new_unchecked(
                            surface.wayland_display as *mut _,
                        ))
                    },
                    _subsurface: Some(subsurface_arc.clone()),
                },
                Some(subsurface_arc),
            )
        }
        Err(e) => {
            log::warn!(
                "Failed to create subsurface ({}), using parent surface directly (may cause issues)",
                e
            );
            (
                WaylandSurfaceWrapper {
                    window_handle: unsafe {
                        WaylandWindowHandle::new(NonNull::new_unchecked(surface.view as *mut _))
                    },
                    display_handle: unsafe {
                        WaylandDisplayHandle::new(NonNull::new_unchecked(
                            surface.wayland_display as *mut _,
                        ))
                    },
                    _subsurface: None,
                },
                None,
            )
        }
    };

    // Create WindowWrapper and RawHandleWrapper
    let window_wrapper = WindowWrapper::new(surface_wrapper);
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

    log::info!("Embedded Linux/Wayland window created successfully");
    subsurface_handle
}

/// Create a Wayland subsurface for Bevy to render into
fn create_subsurface(surface: &SurfaceInfo) -> Result<(*const c_void, WaylandSubsurface), String> {
    // Connect to the Wayland display using the foreign display pointer
    let backend = unsafe { Backend::from_foreign_display(surface.wayland_display as *mut _) };
    let conn = Connection::from_backend(backend);

    // Initialize the registry
    let (globals, queue) = registry_queue_init::<RegistryState>(&conn)
        .map_err(|e| format!("Failed to initialize registry: {}", e))?;

    let qh = queue.handle();

    // Bind compositor and subcompositor
    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .map_err(|e| format!("Failed to bind compositor: {}", e))?;

    let subcompositor: wl_subcompositor::WlSubcompositor = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("Failed to bind subcompositor: {}", e))?;

    // Create a new surface for Bevy
    let bevy_surface = compositor.create_surface(&qh, ());

    // Wrap the parent surface
    let parent_surface_id = unsafe {
        wayland_client::backend::ObjectId::from_ptr(
            wl_surface::WlSurface::interface(),
            surface.view as *mut _,
        )
        .map_err(|e| format!("Failed to create parent surface ID: {:?}", e))?
    };

    let parent_surface = wl_surface::WlSurface::from_id(&conn, parent_surface_id)
        .map_err(|e| format!("Failed to wrap parent surface: {:?}", e))?;

    // Create a subsurface
    let subsurface = subcompositor.get_subsurface(&bevy_surface, &parent_surface, &qh, ());

    // Position relative to parent surface and set to desync mode for independent rendering
    subsurface.set_position(surface.x, surface.y);
    subsurface.set_desync();

    // Make the surface input-transparent by setting an empty input region
    // This allows input events to pass through to the GTK widget underneath
    let empty_region = compositor.create_region(&qh, ());
    bevy_surface.set_input_region(Some(&empty_region));
    bevy_surface.commit();

    // Flush the connection
    conn.flush()
        .map_err(|e| format!("Failed to flush connection: {}", e))?;

    // Get the raw pointer for the Bevy surface
    let bevy_surface_ptr = bevy_surface.id().as_ptr() as *const c_void;

    Ok((
        bevy_surface_ptr,
        WaylandSubsurface {
            _surface: bevy_surface,
            subsurface,
            connection: conn,
        },
    ))
}

/// Handle a mouse button event from Linux
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
/// - `button`: 0 = Left, 1 = Right, 2 = Middle
/// - `pressed`: true if pressed, false if released
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_linux_mouse_button(
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

/// Handle a mouse move event from Linux
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_linux_mouse_moved(app: *mut c_void, x: f32, y: f32) {
    if app.is_null() {
        return;
    }

    let app = &mut *(app as *mut App);

    let mut input_events = app.world_mut().resource_mut::<EmbeddedInputEvents>();
    input_events.add_touch_event(EmbeddedTouchEvent {
        phase: TouchPhase::Moved,
        position: Vec2::new(x, y),
        id: 0,
    });
}

/// Handle a resize event from Linux
///
/// # Safety
///
/// - `app` must be a valid pointer to the App
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_linux_resize(
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
pub unsafe extern "C" fn bevy_embedded_linux_send_message(
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
pub unsafe extern "C" fn bevy_embedded_linux_receive_message(
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
