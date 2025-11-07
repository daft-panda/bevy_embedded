# bevy_embedded

Embedded widget support for Bevy Engine on iOS and Android.

This crate provides the ability to embed Bevy as a widget within native applications. Instead of using winit to manage windows and input, the host application provides a surface (CAMetalLayer on iOS, SurfaceView on Android) and forwards input events to Bevy.

## Features

- **EmbeddedPlugin**: Replaces `WinitPlugin` for embedded mode
- **iOS FFI**: C API for integrating with Swift/Objective-C applications
- **Touch Input**: Forward touch events from the host to Bevy's input system
- **Binary Channel**: Bidirectional message passing between Bevy and the host application
- **Window Handle Injection**: Provide pre-created rendering surfaces to Bevy

## Architecture

The embedded architecture works as follows:

1. The host application (SwiftUI/Android) creates a rendering surface
2. The surface handle is passed to Bevy via FFI
3. Bevy creates a `Window` entity with the injected `RawHandleWrapper`
4. The host forwards touch/input events to Bevy via FFI
5. Bevy renders to the provided surface
6. Bidirectional messaging allows communication between Bevy and the host

## iOS Usage

### Rust Side

```rust
use bevy::prelude::*;
use bevy_embedded::EmbeddedPlugin;

#[no_mangle]
pub extern "C" fn create_bevy_app() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            EmbeddedPlugin,
            // Add your game plugins here
        ))
        .run();
}
```

### Swift Side

```swift
import Metal
import MetalKit

class BevyView: MTKView {
    var bevyContext: UnsafeMutableRawPointer?

    override init(frame: CGRect, device: MTLDevice?) {
        super.init(frame: frame, device: device)

        // Initialize Bevy with the Metal layer
        if let layer = self.layer as? CAMetalLayer {
            bevyContext = bevy_embedded_ios_init(
                Unmanaged.passUnretained(layer).toOpaque(),
                UInt32(frame.width * contentScaleFactor),
                UInt32(frame.height * contentScaleFactor),
                Float(contentScaleFactor)
            )
        }
    }

    override func draw(_ rect: CGRect) {
        bevy_embedded_ios_update(bevyContext)
    }

    // Forward touch events
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let location = touch.location(in: self)
            bevy_embedded_ios_touch_event(
                bevyContext,
                0, // Started
                Float(location.x),
                Float(location.y),
                UInt64(touch.hash)
            )
        }
    }
}
```

## Binary Channel Communication

Send messages from Swift to Bevy:

```swift
let data = "Hello from Swift".data(using: .utf8)!
data.withUnsafeBytes { ptr in
    bevy_embedded_ios_send_message(bevyContext, ptr.baseAddress, data.count)
}
```

Receive messages from Bevy:

```swift
var dataPtr: UnsafePointer<UInt8>?
var length: Int = 0
if bevy_embedded_ios_receive_message(bevyContext, &dataPtr, &length) {
    let data = Data(bytes: dataPtr!, count: length)
    // Process data
    bevy_embedded_ios_free_message(dataPtr, length)
}
```

## Android Usage

### Rust Side

Using the high-level trait API (recommended):

```rust
use bevy::prelude::*;
use bevy_embedded::prelude::*;

struct MyEmbeddedApp;

impl EmbeddedApp for MyEmbeddedApp {
    fn setup(app: &mut App) {
        app.add_plugins(
            DefaultPlugins.build()
                .disable::<bevy::winit::WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    ..Default::default()
                })
        )
        .add_systems(Startup, setup_scene);
        // Add your game systems here
    }
}

// This macro automatically handles asset source configuration for Android
export_embedded_app!(MyEmbeddedApp);

fn setup_scene(mut commands: Commands) {
    // Your scene setup
}
```

Or using the low-level FFI directly:

```rust
use bevy::prelude::*;

#[no_mangle]
pub extern "C" fn bevy_embedded_create_app() -> *mut App {
    let mut app = App::new();

    // IMPORTANT: Configure the embedded asset reader BEFORE adding DefaultPlugins!
    #[cfg(target_os = "android")]
    bevy_embedded::android::configure_embedded_asset_source(&mut app);

    app.add_plugins(DefaultPlugins);
    // Add your game plugins here

    Box::into_raw(Box::new(app))
}

#[no_mangle]
pub extern "C" fn bevy_embedded_update(app: *mut App) {
    unsafe {
        (*app).update();
    }
}

#[no_mangle]
pub extern "C" fn bevy_embedded_destroy(app: *mut App) {
    unsafe {
        let _ = Box::from_raw(app);
    }
}
```

### Kotlin/Java Side

```kotlin
class BevyView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    private var bevyAppPtr: Long = 0

    init {
        holder.addCallback(this)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        bevyAppPtr = BevyNative.nativeCreateApp(
            context as Activity,
            holder.surface,
            width,
            height,
            resources.displayMetrics.density
        )

        // Start render loop
        startRenderLoop()
    }

    private fun startRenderLoop() {
        thread {
            while (bevyAppPtr != 0L) {
                BevyNative.nativeUpdate(bevyAppPtr)
            }
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val phase = when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> 0
            MotionEvent.ACTION_MOVE -> 1
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> 2
            else -> return false
        }

        BevyNative.nativeTouchEvent(
            bevyAppPtr,
            phase,
            event.x,
            event.y,
            event.getPointerId(event.actionIndex).toLong()
        )
        return true
    }
}
```

### Android Asset Loading

For embedded Android contexts (widgets), the default Bevy asset system doesn't work because it requires `ANDROID_APP` which is only available in native activity mode. This crate provides a custom `EmbeddedAndroidAssetReader` that:

1. Uses the `AssetManager` directly via JNI
2. Works in embedded widget contexts
3. Bypasses the need for `ANDROID_APP`

**When using the `export_embedded_app!` macro**, asset loading is automatically configured for you. The macro ensures the custom asset reader is registered before any plugins are added.

**If using the low-level FFI directly**, you MUST call `configure_embedded_asset_source()` before adding `DefaultPlugins`:

```rust
#[cfg(target_os = "android")]
bevy_embedded::android::configure_embedded_asset_source(&mut app);

app.add_plugins(DefaultPlugins); // Will use the custom reader
```

The asset reader is automatically initialized in the JNI layer with the `AssetManager` obtained from the Android Activity.

## Limitations

- Requires the host application to manage the render loop
- No window management features (fullscreen, resize, etc.) - controlled by host
- Limited to touch input (mouse/keyboard support can be added)
- Android: Custom asset reader required for embedded contexts (automatically handled by this crate)

## Examples

See `examples/mobile_embedded` for a complete iOS example with SwiftUI integration.
