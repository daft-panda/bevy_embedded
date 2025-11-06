//! Trait-based API for creating embedded Bevy applications
//!
//! This module provides a trait-based interface that hides FFI entry points
//! from the user. Instead of manually defining FFI functions, users implement
//! the `EmbeddedApp` trait and use the `export_embedded_app!` macro.

use bevy::app::App;

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

            // Add the EmbeddedPlugin first so it can create the window before RenderPlugin builds
            app.add_plugins($crate::EmbeddedPlugin);

            // Create the window by requesting it from the host before adding other plugins
            #[cfg(target_os = "ios")]
            $crate::ios::create_window_from_host(&mut app);

            #[cfg(target_os = "android")]
            $crate::android::create_window_from_host(&mut app);

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
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn bevy_embedded_update(app: *mut bevy::app::App) {
            use bevy::app::PluginsState;
            use bevy::tasks::tick_global_task_pools_on_main_thread;
            use bevy::time::{TimeUpdateStrategy, TimeSender};
            use bevy::platform::time::Instant;

            if !app.is_null() {
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

                    // Send current time to render world if available
                    let now = Instant::now();
                    if let Some(time_sender) = app.world().get_resource::<TimeSender>() {
                        let _ = time_sender.0.try_send(now);
                    }

                    app.update();
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
