# Building the iOS Embedded Example

## Architecture

The new architecture works like WinitPlugin:

1. **Example app** (`src/lib.rs`) exports FFI functions:
   - `bevy_embedded_create_app()` - Creates the Bevy App with EmbeddedPlugin
   - `bevy_embedded_update()` - Updates the app each frame
   - `bevy_embedded_destroy()` - Cleans up the app

2. **EmbeddedPlugin** (`bevy_embedded` crate):
   - Implements `Plugin::finish()` to create the window
   - Calls back to Swift's `bevy_embedded_get_surface()` to get the MTKView

3. **Swift** (`BevyMetalView.swift`):
   - Provides `bevy_embedded_get_surface()` callback
   - Calls Rust FFI functions to manage the app lifecycle

## Build Steps

### 1. Build the Rust Library

```bash
cd examples/mobile_embedded
cargo rustc --target aarch64-apple-ios-sim --lib -- --emit=obj -C link-arg=-Wl,-undefined,dynamic_lookup
```

The `-undefined,dynamic_lookup` flag allows the library to have undefined symbols (like `bevy_embedded_get_surface`) that will be resolved by Swift at runtime.

### 2. Copy Library to Xcode Project

```bash
cp ../../target/aarch64-apple-ios-sim/debug/libbevy_mobile_embedded_example.dylib test-bevy-embedded/
```

### 3. Add Library to Xcode Project

1. Open `test-bevy-embedded.xcodeproj` in Xcode
2. Select the project in the navigator
3. Select the "test-bevy-embedded" target
4. Go to "Build Phases" tab
5. Expand "Link Binary With Libraries"
6. Click the "+" button
7. Click "Add Other..." → "Add Files..."
8. Navigate to and select `libbevy_mobile_embedded_example.dylib`
9. Make sure "Copy items if needed" is UNCHECKED (we'll rebuild it)
10. Click "Add"

### 4. Configure Library Search Path

1. Still in the target settings, go to "Build Settings" tab
2. Search for "Library Search Paths"
3. Add the path: `$(PROJECT_DIR)` (or the absolute path to test-bevy-embedded directory)

### 5. Build and Run

1. Select the iOS Simulator (iPhone 15 Pro recommended)
2. Click Run (Cmd+R)

## Rebuilding After Code Changes

After making changes to Rust code:

```bash
# Rebuild the library
cargo rustc --target aarch64-apple-ios-sim --lib -- --emit=obj -C link-arg=-Wl,-undefined,dynamic_lookup

# Copy to Xcode project
cp ../../target/aarch64-apple-ios-sim/debug/libbevy_mobile_embedded_example.dylib test-bevy-embedded/

# Clean build in Xcode (Shift+Cmd+K) and rebuild
```

## Key Architectural Points

- **Plugin creates window**: EmbeddedPlugin implements `finish()` to create the window by requesting the surface from Swift
- **PrimaryWindow marker**: The window entity needs the `PrimaryWindow` component for RenderPlugin to find it
- **Bidirectional FFI**:
  - Rust exports functions for Swift to call (create, update, destroy)
  - Swift exports `bevy_embedded_get_surface()` for Rust to call during plugin initialization
- **No more unsafe extern "Rust"**: The old pattern of declaring external Rust functions in Rust is removed

## Troubleshooting

**Undefined symbols error**: Make sure the library is added to "Link Binary With Libraries" in Xcode

**Library not found**: Check that the library search path includes `$(PROJECT_DIR)`

**Crashes on startup**: Check that `bevy_embedded_get_surface()` in Swift is properly implemented and returning valid surface info
