//! Trait-based API for creating embedded Bevy applications
//!
//! This module provides a trait-based interface that hides FFI entry points
//! from the user. Instead of manually defining FFI functions, users implement
//! the `EmbeddedApp` trait and use the `export_embedded_app!` macro.

use bevy::app::App;
use std::sync::Mutex;

/// Stores the last error that occurred in the embedded app
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Store an error message from the error handler
#[doc(hidden)]
pub fn store_error(message: String) {
    if let Ok(mut last_error) = LAST_ERROR.lock() {
        *last_error = Some(message);
    }
}

/// Retrieve and clear the last error message
#[doc(hidden)]
pub fn take_last_error() -> Option<String> {
    LAST_ERROR.lock().ok().and_then(|mut e| e.take())
}

/// Trait for defining an embedded Bevy application
///
/// Implement this trait to define your app's behavior, then use the
/// `export_embedded_app!` macro to generate the FFI entry points.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_embedded::prelude::*;
///
/// struct MyEmbeddedApp;
///
/// impl EmbeddedApp for MyEmbeddedApp {
///     fn setup(app: &mut App) {
///         app.add_plugins(DefaultPlugins.build().disable::<WinitPlugin>())
///             .add_systems(Startup, setup_scene);
///     }
/// }
///
/// export_embedded_app!(MyEmbeddedApp);
/// ```
pub trait EmbeddedApp {
    /// Configure the Bevy app with plugins and systems
    ///
    /// This is called once when the app is created by the host.
    /// Add your plugins, systems, and resources here.
    fn setup(app: &mut App);

    /// Optional: Called before the app is created
    ///
    /// Use this for any pre-initialization setup needed before
    /// the App is constructed.
    fn pre_init() {}

    /// Optional: Called after the app is created but before setup
    ///
    /// Use this for any initialization that needs to happen after
    /// the App exists but before plugins are configured.
    fn post_init(_app: &mut App) {}
}

/// Export an embedded app implementation
///
/// This macro generates the necessary FFI entry points for your embedded app.
/// The generated functions are:
/// - `bevy_embedded_create_app()` - Creates and initializes the app
/// - `bevy_embedded_update()` - Updates the app each frame
/// - `bevy_embedded_destroy()` - Cleans up and destroys the app
///
/// # Example
///
/// ```no_run
/// use bevy_embedded::prelude::*;
///
/// struct MyApp;
///
/// impl EmbeddedApp for MyApp {
///     fn setup(app: &mut App) {
///         // Configure your app
///     }
/// }
///
/// export_embedded_app!(MyApp);
/// ```
#[macro_export]
macro_rules! export_embedded_app {
    ($app_type:ty) => {
        /// Entry point that creates and returns the Bevy App
        /// This is called AFTER the host has set up the surface info
        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_embedded_create_app() -> *mut bevy::app::App {
            use bevy::app::{App, PluginsState};
            use bevy::tasks::tick_global_task_pools_on_main_thread;
            use $crate::EmbeddedApp;

            // Call pre-init hook
            <$app_type>::pre_init();

            let mut app = App::new();

            // Set error handler to capture errors from Bevy systems
            app.set_error_handler(|error, context| {
                use std::fmt::Write;
                let mut message = String::new();
                let _ = write!(message, "{}: {}", context, error);
                $crate::store_error(message);
                bevy::log::error!("{}: {}", context, error);
            });

            // Add the EmbeddedPlugin first so it can create the window before RenderPlugin builds
            app.add_plugins($crate::EmbeddedPlugin);

            // Create the window by requesting it from the host before adding other plugins
            #[cfg(target_os = "ios")]
            $crate::ios::create_window_from_host(&mut app);

            #[cfg(target_os = "android")]
            $crate::android::create_window_from_host(&mut app);

            // Configure embedded asset source for Android (must be before plugins)
            #[cfg(target_os = "android")]
            $crate::android::configure_embedded_asset_source(&mut app);

            // Call post-init hook
            <$app_type>::post_init(&mut app);

            // User-defined setup
            <$app_type>::setup(&mut app);

            // Finish and cleanup to initialize all plugins
            app.finish();
            app.cleanup();

            Box::into_raw(Box::new(app))
        }

        /// Update the app (called every frame by host)
        /// Returns 0 on success, non-zero error code if the app should exit with an error
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn bevy_embedded_update(app: *mut bevy::app::App) -> u8 {
            use bevy::app::PluginsState;
            use bevy::tasks::tick_global_task_pools_on_main_thread;

            if app.is_null() {
                $crate::store_error("Null app pointer".to_string());
                return 1;
            }

            unsafe {
                let app = &mut *app;

                let plugins_state = app.plugins_state();
                if plugins_state != PluginsState::Cleaned {
                    while app.plugins_state() == PluginsState::Adding {
                        tick_global_task_pools_on_main_thread();
                    }
                    app.finish();
                    app.cleanup();
                }

                // Update the app
                app.update();

                // Check if the app should exit (e.g., render thread crashed)
                if let Some(exit) = app.should_exit() {
                    if exit.is_error() {
                        // If we don't have a stored error message, create a generic one
                        if $crate::take_last_error().is_none() {
                            $crate::store_error("Bevy app exited with an error".to_string());
                        }
                        bevy::log::error!("App exiting with error: {:?}", exit);
                        return match exit {
                            bevy::app::AppExit::Error(code) => code.get(),
                            _ => 1,
                        };
                    }
                }

                // Check if an error was stored during the update (without AppExit)
                if $crate::take_last_error().is_some() {
                    return 1;
                }

                0 // Success
            }
        }

        /// Get the last error message (if any) and clear it
        /// Returns a pointer to a C string, or null if no error
        /// The caller is responsible for freeing the returned string with bevy_embedded_free_error
        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_embedded_get_last_error() -> *mut std::os::raw::c_char {
            use std::ffi::CString;

            if let Some(error) = $crate::take_last_error() {
                if let Ok(c_string) = CString::new(error) {
                    return c_string.into_raw();
                }
            }
            std::ptr::null_mut()
        }

        /// Free an error string returned by bevy_embedded_get_last_error
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn bevy_embedded_free_error(error: *mut std::os::raw::c_char) {
            if !error.is_null() {
                unsafe {
                    let _ = std::ffi::CString::from_raw(error);
                }
            }
        }

        /// Cleanup and destroy the app
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn bevy_embedded_destroy(app: *mut bevy::app::App) {
            if !app.is_null() {
                unsafe {
                    let _ = Box::from_raw(app);
                }
            }
        }
    };
}
