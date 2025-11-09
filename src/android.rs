//! Android-specific embedded integration with JNI functions
use crate::HostChannel;
use bevy::{
    app::App,
    asset::{
        AssetApp,
        io::{
            AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader,
            VecReader,
        },
    },
    log::info,
    math::Vec2,
    window::{
        PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, Window, WindowResolution,
        WindowWrapper,
    },
};
use futures_lite::stream;
use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject},
    sys::{jbyteArray, jfloat, jint, jlong},
};
use log::{debug, error};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};
use std::{
    ffi::{CString, c_void},
    ptr::NonNull,
    sync::{Arc, Mutex, OnceLock},
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
    window_handle: AndroidNdkWindowHandle,
    display_handle: AndroidDisplayHandle,
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
            error!("No Android surface available yet");
            return;
        }
    };

    if surface_info.native_window.is_null() {
        error!("Host did not provide a valid surface");
        return;
    }

    debug!(
        "Creating embedded Android window: {}x{} @ {}x scale",
        surface_info.width, surface_info.height, surface_info.scale_factor
    );

    // Create the window wrapper for raw-window-handle
    let window_handle = AndroidNdkWindowHandle::new(
        NonNull::new(surface_info.native_window).expect("Native window pointer is null"),
    );

    let display_handle = AndroidDisplayHandle::new();

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

    debug!("Embedded Android window created successfully");
}

// ============================================================================
// Embedded Android Asset Reader
// ============================================================================

/// Custom AssetReader for embedded Android contexts that uses AssetManager directly
/// without requiring ANDROID_APP
pub struct EmbeddedAndroidAssetReader {
    asset_manager: Arc<ndk::asset::AssetManager>,
}

impl EmbeddedAndroidAssetReader {
    /// Create a new reader from an AssetManager pointer
    ///
    /// # Safety
    /// The asset_manager_ptr must be a valid AAssetManager pointer that will remain
    /// valid for the lifetime of this reader
    pub unsafe fn new(asset_manager_ptr: *mut ndk_sys::AAssetManager) -> Self {
        let asset_manager = unsafe {
            ndk::asset::AssetManager::from_ptr(
                NonNull::new(asset_manager_ptr).expect("AssetManager pointer is null"),
            )
        };
        Self {
            asset_manager: Arc::new(asset_manager),
        }
    }
}

impl AssetReader for EmbeddedAndroidAssetReader {
    async fn read<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let path_cstr = CString::new(path.to_str().unwrap())
            .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?;

        let mut opened_asset = self
            .asset_manager
            .open(&path_cstr)
            .ok_or(AssetReaderError::NotFound(path.to_path_buf()))?;

        let bytes = opened_asset
            .buffer()
            .map_err(|e| AssetReaderError::Io(Arc::new(e)))?;

        let reader = VecReader::new(bytes.to_vec());
        Ok(Box::new(reader))
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        // Construct meta path manually (path + ".meta")
        let mut meta_path = path.to_path_buf();
        let mut extension = meta_path
            .extension()
            .map(|e| e.to_os_string())
            .unwrap_or_default();
        extension.push(".meta");
        meta_path.set_extension(extension);
        let path_cstr = CString::new(meta_path.to_str().unwrap())
            .map_err(|_| AssetReaderError::NotFound(meta_path.clone()))?;

        let mut opened_asset = self
            .asset_manager
            .open(&path_cstr)
            .ok_or(AssetReaderError::NotFound(meta_path))?;

        let bytes = opened_asset
            .buffer()
            .map_err(|e| AssetReaderError::Io(Arc::new(e)))?;

        let reader = VecReader::new(bytes.to_vec());
        Ok(Box::new(reader))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let path_cstr = CString::new(path.to_str().unwrap())
            .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?;

        let opened_assets_dir = self
            .asset_manager
            .open_dir(&path_cstr)
            .ok_or(AssetReaderError::NotFound(path.to_path_buf()))?;

        let mapped_stream: Vec<_> = opened_assets_dir
            .filter_map(move |f| {
                let file_path = path.join(std::path::Path::new(f.to_str().unwrap()));
                // Filter out meta files as they are not considered assets
                if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("meta") {
                        return None;
                    }
                }
                Some(file_path.to_owned())
            })
            .collect();

        let read_dir: Box<PathStream> = Box::new(stream::iter(mapped_stream));
        Ok(read_dir)
    }

    async fn is_directory<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Result<bool, AssetReaderError> {
        let cpath = CString::new(path.to_str().unwrap())
            .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?;

        // Check if path exists as a directory
        let _ = self
            .asset_manager
            .open_dir(&cpath)
            .ok_or(AssetReaderError::NotFound(path.to_path_buf()))?;

        // If open (as file) fails, it's a directory
        Ok(self.asset_manager.open(&cpath).is_none())
    }
}

/// Global storage for the embedded asset reader
static EMBEDDED_ASSET_READER: OnceLock<EmbeddedAndroidAssetReader> = OnceLock::new();

/// Initialize the embedded asset reader with the given AssetManager
///
/// # Safety
/// The asset_manager_ptr must be a valid AAssetManager pointer
pub unsafe fn init_embedded_asset_reader(asset_manager_ptr: *mut ndk_sys::AAssetManager) {
    let reader = unsafe { EmbeddedAndroidAssetReader::new(asset_manager_ptr) };
    let _ = EMBEDDED_ASSET_READER.set(reader);
}

/// Get the embedded asset reader, if initialized
pub fn get_embedded_asset_reader() -> Option<&'static EmbeddedAndroidAssetReader> {
    EMBEDDED_ASSET_READER.get()
}

/// Configure the Bevy app to use the embedded Android asset reader
///
/// **IMPORTANT**: Call this BEFORE adding AssetPlugin/DefaultPlugins to your app!
///
/// This function replaces the default Android asset reader with our custom
/// embedded reader that works in embedded widget contexts.
///
/// # Example
/// ```ignore
/// use bevy::prelude::*;
/// use bevy_embedded::android::configure_embedded_asset_source;
///
/// #[no_mangle]
/// pub extern "C" fn bevy_embedded_create_app() -> *mut App {
///     let mut app = App::new();
///
///     // MUST be called before DefaultPlugins!
///     #[cfg(target_os = "android")]
///     configure_embedded_asset_source(&mut app);
///
///     app.add_plugins(DefaultPlugins);
///     // ... rest of app setup
///
///     Box::into_raw(Box::new(app))
/// }
/// ```
#[cfg(target_os = "android")]
pub fn configure_embedded_asset_source(app: &mut App) {
    // Get the embedded asset reader
    let reader = get_embedded_asset_reader()
        .expect("Embedded asset reader must be initialized before configuring Bevy app");

    // Clone the Arc so we can share it with the closure
    let asset_manager = reader.asset_manager.clone();

    // Create a custom asset source that uses our embedded reader
    let source = AssetSourceBuilder::default().with_reader(move || {
        Box::new(EmbeddedAndroidAssetReader {
            asset_manager: asset_manager.clone(),
        })
    });

    // Register it as the default source using the proper API
    app.register_asset_source(AssetSourceId::Default, source);
}

// ============================================================================
// android_main entry point
// ============================================================================

/// android_main entry point - called by android-activity crate
/// This initializes the ANDROID_APP global so Bevy's asset system works
#[unsafe(no_mangle)]
fn android_main(android_app: bevy::android::android_activity::AndroidApp) {
    // Set ANDROID_APP for Bevy's asset system
    let _ = bevy::android::ANDROID_APP.set(android_app);
    info!("android_main: Initialized ANDROID_APP for Bevy asset system");

    // We don't run the app here - that happens via JNI calls
    // Just keep the activity alive
    info!("android_main: Activity initialized, waiting for JNI calls...");
}

// ============================================================================
// JNI Entry Points
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeCreateApp(
    mut env: JNIEnv,
    _class: JClass,
    activity: JObject,
    surface: JObject,
    width: jint,
    height: jint,
    scale_factor: jfloat,
) -> jlong {
    debug!(
        "nativeCreateApp called: {}x{} @ {}x",
        width, height, scale_factor
    );

    // Get AssetManager from Activity
    let asset_manager_ptr = unsafe {
        let assets_obj = env
            .call_method(
                &activity,
                "getAssets",
                "()Landroid/content/res/AssetManager;",
                &[],
            )
            .expect("Failed to get AssetManager")
            .l()
            .expect("AssetManager is null");
        ndk_sys::AAssetManager_fromJava(env.get_raw(), assets_obj.as_raw())
    };

    if asset_manager_ptr.is_null() {
        error!("Failed to get AssetManager from Activity");
        return 0;
    } else {
        debug!("Got AssetManager: {:p}", asset_manager_ptr);
    }

    // Initialize ndk-context for JNI calls
    unsafe {
        let vm = env.get_java_vm().unwrap().get_java_vm_pointer() as *mut c_void;
        let activity_ptr = activity.as_raw() as *mut c_void;
        ndk_context::initialize_android_context(vm, activity_ptr);
    }

    // Initialize our custom embedded asset reader
    unsafe {
        init_embedded_asset_reader(asset_manager_ptr);
    }
    debug!("Initialized embedded asset reader");

    // Get ANativeWindow from Surface
    let native_window_ptr = unsafe {
        let surface_ptr = surface.as_raw();
        ndk_sys::ANativeWindow_fromSurface(env.get_raw(), surface_ptr)
    };

    if native_window_ptr.is_null() {
        error!("Failed to get native window from surface");
        return 0;
    }

    debug!("Got native window pointer: {:p}", native_window_ptr);

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
        error!("Failed to create Bevy app");
        return 0;
    }

    debug!("Bevy app created successfully: {:p}", app_ptr);
    app_ptr as jlong
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeUpdate(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
) -> jint {
    if app_ptr == 0 {
        return 1; // Error: null app pointer
    }

    unsafe extern "C" {
        fn bevy_embedded_update(app: *mut App) -> u8;
    }

    unsafe { bevy_embedded_update(app_ptr as *mut App) as jint }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeGetLastError<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
) -> JObject<'local> {
    use std::ffi::CStr;

    unsafe extern "C" {
        fn bevy_embedded_get_last_error() -> *mut std::os::raw::c_char;
        fn bevy_embedded_free_error(error: *mut std::os::raw::c_char);
    }

    unsafe {
        let error_ptr = bevy_embedded_get_last_error();
        if error_ptr.is_null() {
            return JObject::null();
        }

        let error_cstr = CStr::from_ptr(error_ptr);
        let error_str = error_cstr
            .to_str()
            .unwrap_or("Invalid UTF-8 in error message");

        // Convert to Java String and return
        let result = match env.new_string(error_str) {
            Ok(jstring) => jstring.into(),
            Err(_) => JObject::null(),
        };

        bevy_embedded_free_error(error_ptr);
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_bevyembedded_BevyNative_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    app_ptr: jlong,
) {
    if app_ptr == 0 {
        return;
    }

    debug!("Destroying Bevy app");

    unsafe extern "C" {
        fn bevy_embedded_destroy(app: *mut App);
    }

    unsafe {
        bevy_embedded_destroy(app_ptr as *mut App);
    }

    debug!("Bevy app destroyed");
}

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
            let mut input_events = app_ref
                .world_mut()
                .resource_mut::<crate::EmbeddedInputEvents>();
            input_events.add_touch_event(crate::EmbeddedTouchEvent {
                phase: touch_phase,
                position: Vec2::new(x as f32, y as f32),
                id: id as u64,
            });
        }
    }
}

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

    debug!(
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
            error!("Failed to convert byte array: {:?}", e);
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
                        error!("Failed to create byte array: {:?}", e);
                    }
                }
            }
        }
    }

    JObject::null().into_raw() as jbyteArray
}
