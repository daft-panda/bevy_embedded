//! WASM-specific functionality for embedded Bevy applications
//!
//! This module provides the WASM entry point and host channel implementation
//! using JavaScript interop via wasm-bindgen.

use bevy::app::App;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use bevy::window::{PresentMode, Window, WindowResolution};
use std::cell::RefCell;
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;

use crate::input::{EmbeddedInputEvents, EmbeddedTouchEvent, TouchPhase};

// Note: bevy_embedded_resize is intentionally not provided - Bevy's web backend
// handles canvas resize automatically via the winit integration.

thread_local! {
    static WASM_APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static HOST_TO_BEVY: RefCell<VecDeque<Vec<u8>>> = const { RefCell::new(VecDeque::new()) };
    static BEVY_TO_HOST: RefCell<VecDeque<Vec<u8>>> = const { RefCell::new(VecDeque::new()) };
}

/// WASM-specific host channel that uses thread-local queues
#[derive(Resource, Default)]
pub struct WasmHostChannel;

impl WasmHostChannel {
    /// Send a message to the host (JavaScript)
    pub fn send(&self, data: Vec<u8>) {
        BEVY_TO_HOST.with(|queue| {
            queue.borrow_mut().push_back(data);
        });
    }

    /// Receive a message from the host (non-blocking)
    pub fn receive(&self) -> Option<Vec<u8>> {
        HOST_TO_BEVY.with(|queue| queue.borrow_mut().pop_front())
    }
}

/// JavaScript-callable function to send a message from host to Bevy
#[wasm_bindgen]
pub fn bevy_embedded_send_message(data: &[u8]) {
    HOST_TO_BEVY.with(|queue| {
        queue.borrow_mut().push_back(data.to_vec());
    });
}

/// JavaScript-callable function to receive a message from Bevy
/// Returns null if no message is available
#[wasm_bindgen]
pub fn bevy_embedded_receive_message() -> Option<Vec<u8>> {
    BEVY_TO_HOST.with(|queue| queue.borrow_mut().pop_front())
}

/// JavaScript-callable function to send a touch event
#[wasm_bindgen]
pub fn bevy_embedded_touch_event(id: u64, phase: u8, x: f32, y: f32) {
    let phase = match phase {
        0 => TouchPhase::Started,
        1 => TouchPhase::Moved,
        2 => TouchPhase::Ended,
        _ => TouchPhase::Cancelled,
    };

    WASM_APP.with(|app_cell| {
        if let Some(app) = app_cell.borrow_mut().as_mut() {
            if let Some(mut input_events) =
                app.world_mut().get_resource_mut::<EmbeddedInputEvents>()
            {
                input_events.touch_events.push(EmbeddedTouchEvent {
                    id,
                    phase,
                    position: Vec2::new(x, y),
                });
            }
        }
    });
}

/// JavaScript-callable function to update the app (called every frame)
/// Returns true if the app is still running, false if it should exit
#[wasm_bindgen]
pub fn bevy_embedded_update() -> bool {
    WASM_APP.with(|app_cell| {
        if let Some(app) = app_cell.borrow_mut().as_mut() {
            app.update();

            // Check if the app should exit
            if let Some(exit) = app.should_exit() {
                if exit.is_error() {
                    log::error!("Bevy app exiting with error: {:?}", exit);
                    return false;
                }
            }
            true
        } else {
            false
        }
    })
}

/// JavaScript-callable function to destroy the app
#[wasm_bindgen]
pub fn bevy_embedded_destroy() {
    WASM_APP.with(|app_cell| {
        app_cell.borrow_mut().take();
    });
}

/// Create a window for WASM embedded mode with a canvas selector
pub fn create_window_from_host_with_canvas(app: &mut App, width: u32, height: u32, canvas: String) {
    let window = Window {
        resolution: WindowResolution::new(width, height),
        present_mode: PresentMode::AutoVsync,
        title: "Bevy Embedded".to_string(),
        canvas: Some(canvas),
        ..Default::default()
    };

    app.world_mut().spawn(window);
}

/// Create a window for WASM embedded mode (uses default canvas selector "#bevy")
pub fn create_window_from_host(app: &mut App, width: u32, height: u32) {
    create_window_from_host_with_canvas(app, width, height, "#bevy".to_string());
}

/// Store the app in thread-local storage for WASM
pub fn store_app(app: App) {
    WASM_APP.with(|app_cell| {
        *app_cell.borrow_mut() = Some(app);
    });
}

/// Get mutable access to the stored app
pub fn with_app<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut App) -> R,
{
    WASM_APP.with(|app_cell| app_cell.borrow_mut().as_mut().map(f))
}

/// Configure the Bevy app to use a custom asset path prefix for WASM.
///
/// On WASM, Bevy loads assets via HTTP requests relative to the document location.
/// By default, it uses the "assets" folder. This function allows you to specify
/// a different path prefix (e.g., "https://cdn.example.com/assets" or "/game/assets").
///
/// **IMPORTANT**: Call this BEFORE adding `DefaultPlugins` or `AssetPlugin` to your app!
///
/// # Example
/// ```ignore
/// use bevy::prelude::*;
/// use bevy_embedded::wasm::configure_wasm_asset_path;
///
/// fn setup_app(app: &mut App) {
///     // Load assets from a CDN or different path
///     configure_wasm_asset_path(app, "https://cdn.example.com/assets");
///
///     app.add_plugins(DefaultPlugins);
///     // ...
/// }
/// ```
pub fn configure_wasm_asset_path(app: &mut App, path: impl Into<String>) {
    use bevy::asset::{AssetApp, io::AssetSourceBuilder, io::AssetSourceId};

    let path = path.into();

    // Create a custom asset source that uses our specified path
    let source = AssetSourceBuilder::default()
        .with_reader(move || Box::new(bevy::asset::io::wasm::HttpWasmAssetReader::new(&path)));

    // Register it as the default source (must be done before AssetPlugin is added)
    app.register_asset_source(AssetSourceId::Default, source);
}
