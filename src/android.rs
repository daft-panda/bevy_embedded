//! Android-specific embedded integration with JNI functions

use crate::HostChannel;
use bevy::{
    app::App,
    log::info,
    math::Vec2,
    window::{PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, Window, WindowResolution, WindowWrapper},
};
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "android")]
use jni::{
    objects::{JByteArray, JClass, JObject},
    sys::{jbyteArray, jfloat, jint, jlong},
    JNIEnv,
};


/// Android surface information passed from Java/Kotlin
#[repr(C)]
pub struct AndroidSurfaceInfo {
    pub native_window: *mut std::ffi::c_void,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

// SAFETY: The native_window pointer is only accessed from the main thread
// and is immediately consumed when creating the window.
unsafe impl Send for AndroidSurfaceInfo {}
unsafe impl Sync for AndroidSurfaceInfo {}

use std::sync::OnceLock;

/// Global storage for the current surface being initialized
static CURRENT_SURFACE: OnceLock<Mutex<Option<AndroidSurfaceInfo>>> = OnceLock::new();

/// Called by Rust to retrieve the surface info
pub fn get_android_surface() -> Option<AndroidSurfaceInfo> {
    CURRENT_SURFACE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .take()
}

/// Sets the current Android surface (called before app creation)
pub fn set_android_surface(surface: AndroidSurfaceInfo) {
    *CURRENT_SURFACE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap() = Some(surface);
}

/// Wrapper for the Android native window that implements the required traits
struct AndroidWindowWrapper {
    window_handle: raw_window_handle::AndroidNdkWindowHandle,
    display_handle: raw_window_handle::AndroidDisplayHandle,
}

unsafe impl Send for AndroidWindowWrapper {}
unsafe impl Sync for AndroidWindowWrapper {}

impl HasWindowHandle for AndroidWindowWrapper {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::WindowHandle::borrow_raw(
                RawWindowHandle::AndroidNdk(self.window_handle),
            ))
        }
    }
}

impl HasDisplayHandle for AndroidWindowWrapper {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(raw_window_handle::DisplayHandle::borrow_raw(
                RawDisplayHandle::Android(self.display_handle),
            ))
        }
    }
}

/// Called by EmbeddedPlugin during finish() to create the window from Android surface
pub fn create_window_from_host(app: &mut App) {
    let surface_info = match get_android_surface() {
        Some(info) => info,
        None => {
            info!("No Android surface available yet");
            return;
        }
    };

    if surface_info.native_window.is_null() {
        info!("Host did not provide a valid surface");
        return;
    }

    info!(
        "Creating embedded Android window: {}x{} @ {}x scale",
        surface_info.width, surface_info.height, surface_info.scale_factor
    );

    #[cfg(target_os = "android")]
    {
        use std::ptr::NonNull;

        // Create the window wrapper for raw-window-handle
        let window_handle = raw_window_handle::AndroidNdkWindowHandle::new(
            NonNull::new(surface_info.native_window).expect("Native window pointer is null")
        );

        let display_handle = raw_window_handle::AndroidDisplayHandle::new();

        let android_wrapper = AndroidWindowWrapper {
            window_handle,
            display_handle,
        };

        // Create WindowWrapper and RawHandleWrapper
        let window_wrapper = WindowWrapper::new(android_wrapper);
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

        info!("Embedded Android window created successfully");
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = surface_info; // Suppress unused variable warning
    }
}

// ============================================================================
// Dummy android_main for compatibility
// ============================================================================

/// Dummy android_main to satisfy linker expectations
/// We don't actually use NativeActivity - we use JNI instead
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(_app: *mut std::ffi::c_void) {
    // This should never be called since we use JNI, not NativeActivity
    panic!("android_main should not be called - using JNI interface instead");
}

// ============================================================================
// JNI Entry Points
// ============================================================================

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeCreateApp(
    env: JNIEnv,
    _class: JClass,
    surface: JObject,
    width: jint,
    height: jint,
    scale_factor: jfloat,
) -> jlong {
    use std::ffi::c_void;

    info!("nativeCreateApp called: {}x{} @ {}x", width, height, scale_factor);

    // Initialize ndk-context with the JNI environment
    unsafe {
        let vm = env.get_java_vm().unwrap().get_java_vm_pointer() as *mut std::ffi::c_void;
        let activity = _class.as_raw() as *mut std::ffi::c_void;
        ndk_context::initialize_android_context(vm, activity);
    }

    // Get ANativeWindow from Surface
    let native_window_ptr = unsafe {
        let surface_ptr = surface.as_raw();
        ndk_sys::ANativeWindow_fromSurface(env.get_raw(), surface_ptr)
    };

    if native_window_ptr.is_null() {
        info!("Failed to get native window from surface");
        return 0;
    }

    info!("Got native window pointer: {:?}", native_window_ptr);

    // Store surface info globally so create_window_from_host can access it
    set_android_surface(AndroidSurfaceInfo {
        native_window: native_window_ptr as *mut c_void,
        width: width as u32,
        height: height as u32,
        scale_factor,
    });

    // Call the user's exported bevy_embedded_create_app function
    unsafe extern "C" {
        fn bevy_embedded_create_app() -> *mut App;
    }

    let app_ptr = unsafe { bevy_embedded_create_app() };

    if app_ptr.is_null() {
        info!("Failed to create Bevy app");
        return 0;
    }

    info!("Bevy app created successfully: {:?}", app_ptr);
    app_ptr as jlong
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeUpdate(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
) {
    if app_ptr == 0 {
        return;
    }

    unsafe extern "C" {
        fn bevy_embedded_update(app: *mut App);
    }

    unsafe {
        bevy_embedded_update(app_ptr as *mut App);
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
) {
    if app_ptr == 0 {
        return;
    }

    info!("Destroying Bevy app");

    unsafe extern "C" {
        fn bevy_embedded_destroy(app: *mut App);
    }

    unsafe {
        bevy_embedded_destroy(app_ptr as *mut App);
    }

    info!("Bevy app destroyed");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeTouchEvent(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    id: jlong,
) {
    let app = app_ptr as *mut App;
    if app.is_null() {
        return;
    }

    unsafe {
        let app_ref = &mut *app;

        if let Some(touch_phase) = crate::TouchPhase::from_u8(phase as u8) {
            let mut input_events = app_ref.world_mut().resource_mut::<crate::EmbeddedInputEvents>();
            input_events.add_touch_event(crate::EmbeddedTouchEvent {
                phase: touch_phase,
                position: Vec2::new(x as f32, y as f32),
                id: id as u64,
            });
        }
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeResize(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
    width: jint,
    height: jint,
    scale_factor: jfloat,
) {
    let app = app_ptr as *mut App;
    if app.is_null() {
        return;
    }

    info!(
        "Android resize: {}x{} @ {}x scale",
        width, height, scale_factor
    );

    unsafe {
        if let Some(mut world) = (*app).world_mut().into() {
            if let Some(mut window) = world.query::<&mut Window>().iter_mut(&mut world).next() {
                window.resolution.set(width as f32, height as f32);
                window
                    .resolution
                    .set_scale_factor_override(Some(scale_factor));
            }
        }
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeSendMessage(
    env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
    data: JByteArray,
) {
    let app = app_ptr as *mut App;
    if app.is_null() {
        return;
    }

    // Convert Java byte array to Rust Vec<u8>
    let bytes = match env.convert_byte_array(data) {
        Ok(bytes) => bytes,
        Err(e) => {
            info!("Failed to convert byte array: {:?}", e);
            return;
        }
    };

    unsafe {
        let app_ref = &mut *app;
        if let Some(channel) = app_ref.world().get_resource::<HostChannel>() {
            channel.send(bytes);
        }
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeReceiveMessage(
    env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
) -> jbyteArray {
    let app = app_ptr as *mut App;
    if app.is_null() {
        return JObject::null().into_raw() as jbyteArray;
    }

    unsafe {
        let app_ref = &mut *app;
        if let Some(channel) = app_ref.world().get_resource::<HostChannel>() {
            if let Some(message) = channel.receive() {
                // Convert Rust Vec<u8> to Java byte array
                match env.byte_array_from_slice(&message) {
                    Ok(array) => return array.into_raw(),
                    Err(e) => {
                        info!("Failed to create byte array: {:?}", e);
                    }
                }
            }
        }
    }

    JObject::null().into_raw() as jbyteArray
}
