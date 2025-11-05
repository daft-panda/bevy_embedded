# Xcode Setup Guide for Bevy Embedded

This guide shows you how to integrate Bevy into your iOS app with automatic Rust compilation.

## Prerequisites

- Xcode 15+ installed
- Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- iOS targets installed:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```

## Step 1: Add Build Script to Xcode

1. Open your Xcode project
2. Select your app target
3. Go to **Build Phases**
4. Click **+** → **New Run Script Phase**
5. Drag the script phase **above** "Compile Sources"
6. Name it "Build Rust Library"
7. Add this script:

```bash
bash "${PROJECT_DIR}/../build_rust_deps.sh"
```

Or if you've copied the script into your Xcode project:

```bash
bash "${PROJECT_DIR}/path/to/build_rust_deps.sh"
```

8. Add output files (this helps Xcode cache the build):
   - `$(BUILT_PRODUCTS_DIR)/libbevy_mobile_embedded_example.dylib`

## Step 2: Configure Build Settings

### Disable Script Sandboxing (Required)

The build script creates a symlink for easy library integration. To allow this, you must disable script sandboxing:

1. Select your target
2. Go to **Build Settings**
3. Search for "ENABLE_USER_SCRIPT_SANDBOXING"
4. Set **User Script Sandboxing** to **No**

Without this setting, the script will still build the Rust library but won't create the convenient symlink.

### (Optional) Add User-Defined Setting

By default, the build script assumes the Rust project is in the same directory as the script itself.

**If your build script and Cargo.toml are in the same directory**: No additional configuration needed! Skip this step.

**If they're in different locations**, add a build setting:

1. Select your target
2. Go to **Build Settings**
3. Click **+** → **Add User-Defined Setting**
4. Name: `BEVY_RUST_PROJECT_PATH`
5. Value: Absolute path to your Rust project directory containing Cargo.toml

Example: `/Users/yourusername/path/to/your/rust/project`

**Important**: Use absolute paths, not relative paths like `$(PROJECT_DIR)/..`

### Link the Rust Dynamic Library

The build script creates a symlink in your project directory that automatically points to the correct library (device or simulator).

1. **First, build your project once** (⌘B) - This creates the symlink
2. In your project navigator, drag the symlink file into your Xcode project:
   - The symlink is at: `YourTarget/libbevy_mobile_embedded_example.dylib`
   - You can also navigate there in Finder from your project directory
3. When prompted, ensure:
   - ✅ "Copy items if needed" is **unchecked** (we want to reference the symlink, not copy it)
   - ✅ "Add to targets" has your app target selected
4. Go to your target's **General** tab → **Frameworks, Libraries, and Embedded Content**
5. Find `libbevy_mobile_embedded_example.dylib` in the list
6. Change its setting to **"Embed & Sign"**

The symlink automatically updates to point to the correct library when switching between device and simulator builds - no manual intervention needed!

### Add Required System Frameworks

In **Build Phases** → **Link Binary With Libraries**, add:

- Metal.framework
- MetalKit.framework
- UIKit.framework
- QuartzCore.framework

## Step 3: Add Swift Files

Copy these files to your Xcode project:

1. **BevyMetalView.swift** - The SwiftUI view that hosts Bevy
2. **ContentView.swift** - Example usage (or integrate into your existing views)

## Step 4: Use in Your SwiftUI App

```swift
import SwiftUI

struct ContentView: View {
    @State private var bevyController: BevyViewController?

    var body: some View {
        BevyMetalView(
            controller: $bevyController,
            onMessageReceived: { data in
                // Handle messages from Bevy
                print("Received \(data.count) bytes from Bevy")
            }
        )
        .ignoresSafeArea()
    }
}
```

## Step 5: Build and Run

1. Select a simulator or device
2. Build (⌘B) - The Rust library will compile automatically
3. Run (⌘R)

The build script will:

- ✅ Detect if you're building for simulator or device
- ✅ Build for the correct architecture (arm64, x86_64)
- ✅ Use Debug or Release configuration automatically
- ✅ Create universal libraries when needed
- ✅ Cache builds in Xcode's derived data

## Customization

### Change Library Name

Edit `build_rust_deps.sh`:

```bash
LIBRARY_NAME="your_custom_name"
```

Update linker flags to match: `-lyour_custom_name`

### Custom Cargo Features

Edit the cargo build command in `build_rust_deps.sh`:

```bash
cargo rustc $CARGO_FLAGS --target "$RUST_TARGET" --lib \
    --features "your_features" -- \
    -C link-arg=-Wl,-undefined,dynamic_lookup
```

## Troubleshooting

### "Library not found"

- Ensure the build script ran successfully (check build log)
- Verify `BEVY_RUST_PROJECT_PATH` points to the correct directory
- Check that `Library Search Paths` includes `$(BUILT_PRODUCTS_DIR)`

### "ld: library 'System' not found"

This is a known Rust/Xcode issue. The build script fixes it automatically by resetting `PATH`.
If it still occurs, ensure the build script phase runs **before** compilation.

### Slow Builds

First build will be slow as Rust compiles all dependencies. Subsequent builds are incremental and much faster.

For faster debug builds, you can set in `~/.cargo/config.toml`:

```toml
[profile.dev]
opt-level = 1
```

### Architecture Errors

- For simulator: Ensure you've installed `aarch64-apple-ios-sim` (Apple Silicon) or `x86_64-apple-ios` (Intel)
- For device: Ensure you've installed `aarch64-apple-ios`

## Advanced: XCFramework Distribution

To distribute your Bevy component as an XCFramework:

```bash
# Build for all platforms
./build_ios.sh

# Create XCFramework
xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/libbevy_mobile_embedded_example.dylib \
    -library target/aarch64-apple-ios-sim/release/libbevy_mobile_embedded_example.dylib \
    -library target/x86_64-apple-ios/release/libbevy_mobile_embedded_example.dylib \
    -output BevyEmbedded.xcframework
```

## Example Project Structure

```
YourApp/
├── YourApp.xcodeproj
├── YourApp/
│   ├── ContentView.swift
│   ├── BevyMetalView.swift
│   └── ...
└── rust-project/
    ├── Cargo.toml
    ├── src/
    │   └── lib.rs
    └── build_rust_deps.sh
```

Build setting:

- `BEVY_RUST_PROJECT_PATH = $(PROJECT_DIR)/rust-project`
