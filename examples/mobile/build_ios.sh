#!/usr/bin/env bash

# Build Bevy Embedded for all iOS platforms
# Useful for creating XCFrameworks or testing all targets

set -e

echo "🦀 Building Bevy Embedded for all iOS platforms"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Configuration
LIBRARY_NAME="bevy_mobile_embedded_example"
PROFILE="${1:-release}"  # Default to release, pass "debug" for debug builds

if [ "$PROFILE" = "release" ]; then
    CARGO_FLAGS="--release"
    echo "📦 Building in Release mode"
else
    CARGO_FLAGS=""
    PROFILE="debug"
    echo "📦 Building in Debug mode"
fi

# Targets to build
TARGETS=(
    "aarch64-apple-ios"           # Device (iPhone/iPad)
    "aarch64-apple-ios-sim"       # Simulator (Apple Silicon Mac)
    "x86_64-apple-ios"            # Simulator (Intel Mac)
)

echo "🎯 Targets: ${TARGETS[*]}"

# Install all targets
echo "📥 Ensuring targets are installed..."
for TARGET in "${TARGETS[@]}"; do
    rustup target add "$TARGET" 2>/dev/null || true
done

# Build each target
echo ""
echo "🔨 Building libraries..."
for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Building for $TARGET..."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    cargo rustc $CARGO_FLAGS --target "$TARGET" --lib -- \
        -C link-arg=-Wl,-undefined,dynamic_lookup

    DYLIB_PATH="../../target/$TARGET/$PROFILE/lib${LIBRARY_NAME}.dylib"

    if [ -f "$DYLIB_PATH" ]; then
        # Set the install_name
        install_name_tool -id "@rpath/lib${LIBRARY_NAME}.dylib" "$DYLIB_PATH"

        echo "✅ Built: $DYLIB_PATH"

        # Show library info
        SIZE=$(du -h "$DYLIB_PATH" | cut -f1)
        echo "   Size: $SIZE"
    else
        echo "❌ Failed to build $TARGET"
        exit 1
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🎉 All builds complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📦 Built dynamic libraries:"
for TARGET in "${TARGETS[@]}"; do
    echo "  - ../../target/$TARGET/$PROFILE/lib${LIBRARY_NAME}.dylib"
done

echo ""
echo "💡 To create an XCFramework, run:"
echo ""
echo "  xcodebuild -create-xcframework \\"
echo "      -library ../../target/aarch64-apple-ios/$PROFILE/lib${LIBRARY_NAME}.dylib \\"
echo "      -library ../../target/aarch64-apple-ios-sim/$PROFILE/lib${LIBRARY_NAME}.dylib \\"
echo "      -library ../../target/x86_64-apple-ios/$PROFILE/lib${LIBRARY_NAME}.dylib \\"
echo "      -output BevyEmbedded.xcframework"
echo ""
