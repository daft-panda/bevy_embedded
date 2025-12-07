//! Configuration for embedded Bevy applications
//!
//! This module provides a universal configuration struct that can be passed
//! from the host application to configure the embedded Bevy instance.

use std::ffi::{CStr, c_char, c_void};
use std::sync::Mutex;

/// Configuration passed from the host application to the embedded Bevy app.
///
/// This struct uses C-compatible types for FFI. All pointer fields are optional
/// and can be null.
#[repr(C)]
pub struct EmbeddedConfig {
    /// Pointer to the native view (NSView on macOS, UIView on iOS, etc.)
    pub view: *const c_void,
    /// Width in physical pixels
    pub width: u32,
    /// Height in physical pixels
    pub height: u32,
    /// Scale factor (retina displays have scale > 1.0)
    pub scale_factor: f32,
    /// Asset path override (null-terminated C string, or null for default "assets")
    pub asset_path: *const c_char,
}

impl Default for EmbeddedConfig {
    fn default() -> Self {
        Self {
            view: std::ptr::null(),
            width: 0,
            height: 0,
            scale_factor: 1.0,
            asset_path: std::ptr::null(),
        }
    }
}

// Safety: The view pointer is only accessed from the main thread where
// the Bevy app is created and updated.
unsafe impl Send for EmbeddedConfig {}

/// Global storage for config set by the host before app creation
pub static EMBEDDED_CONFIG: Mutex<Option<EmbeddedConfig>> = Mutex::new(None);

/// Global storage for asset path (extracted from config, kept alive)
pub static ASSET_PATH: Mutex<Option<String>> = Mutex::new(None);

/// Set the embedded configuration from the host application.
///
/// This must be called before `bevy_embedded_create_app()`.
///
/// # Safety
///
/// - `config` must be a valid pointer to an `EmbeddedConfig` struct
/// - The `view` pointer in the config must be valid for the lifetime of the Bevy app
/// - The `asset_path` pointer (if not null) must point to a valid null-terminated string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bevy_embedded_set_config(config: *const EmbeddedConfig) {
    if config.is_null() {
        log::error!("bevy_embedded_set_config called with null config");
        return;
    }

    let config = unsafe { &*config };

    // Extract and store asset path as owned String
    let asset_path = if config.asset_path.is_null() {
        None
    } else {
        unsafe {
            CStr::from_ptr(config.asset_path)
                .to_str()
                .ok()
                .map(|s| s.to_owned())
        }
    };

    if let Ok(mut guard) = ASSET_PATH.lock() {
        *guard = asset_path;
    }

    // Copy the config (asset_path in struct not needed since we stored it separately)
    let stored_config = EmbeddedConfig {
        view: config.view,
        width: config.width,
        height: config.height,
        scale_factor: config.scale_factor,
        asset_path: std::ptr::null(), // We stored it in ASSET_PATH
    };

    if let Ok(mut guard) = EMBEDDED_CONFIG.lock() {
        *guard = Some(stored_config);
    }
}

/// Take the stored config (used internally during app creation)
pub fn take_config() -> Option<EmbeddedConfig> {
    EMBEDDED_CONFIG.lock().ok().and_then(|mut g| g.take())
}

/// Get the stored asset path (does not consume it)
pub fn get_asset_path() -> Option<String> {
    ASSET_PATH.lock().ok().and_then(|g| g.clone())
}
