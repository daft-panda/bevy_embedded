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

## Android Support

Android support is planned but not yet implemented. The architecture will be similar to iOS but using JNI and SurfaceView.

## Limitations

- Currently iOS-only (Android coming soon)
- Requires the host application to manage the render loop
- No window management features (fullscreen, resize, etc.) - controlled by host
- Limited to touch input (mouse/keyboard support can be added)

## Examples

See `examples/mobile_embedded` for a complete iOS example with SwiftUI integration.
