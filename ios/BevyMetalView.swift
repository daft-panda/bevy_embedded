//
//  BevyMetalView.swift
//  Bevy Embedded Example
//

import SwiftUI
import MetalKit

// Surface info struct matching Rust
struct EmbeddedSurfaceInfo {
    var uiView: UnsafeRawPointer?
    var width: UInt32
    var height: UInt32
    var scaleFactor: Float
}

// Global storage for the current surface being initialized
private var currentSurface: EmbeddedSurfaceInfo?

// Callback that Rust calls to get the surface
@_silgen_name("bevy_embedded_get_surface")
func bevyEmbeddedGetSurface(_ out: UnsafeMutablePointer<EmbeddedSurfaceInfo>) {
    if let surface = currentSurface {
        out.pointee = surface
    } else {
        out.pointee = EmbeddedSurfaceInfo(uiView: nil, width: 0, height: 0, scaleFactor: 1.0)
    }
}

// Import FFI functions from the example
@_silgen_name("bevy_embedded_create_app")
func bevyEmbeddedCreateApp() -> UnsafeMutableRawPointer?

@_silgen_name("bevy_embedded_update")
func bevyEmbeddedUpdate(_ app: UnsafeMutableRawPointer)

@_silgen_name("bevy_embedded_destroy")
func bevyEmbeddedDestroy(_ app: UnsafeMutableRawPointer)

// Import FFI functions from bevy_embedded crate
@_silgen_name("bevy_embedded_ios_touch_event")
func bevyEmbeddedIosTouchEvent(_ app: UnsafeMutableRawPointer, _ phase: UInt8, _ x: Float, _ y: Float, _ id: UInt64)

@_silgen_name("bevy_embedded_ios_resize")
func bevyEmbeddedIosResize(_ app: UnsafeMutableRawPointer, _ width: UInt32, _ height: UInt32, _ scaleFactor: Float)

@_silgen_name("bevy_embedded_ios_send_message")
func bevyEmbeddedIosSendMessage(_ app: UnsafeMutableRawPointer, _ data: UnsafePointer<UInt8>, _ length: Int)

@_silgen_name("bevy_embedded_ios_receive_message")
func bevyEmbeddedIosReceiveMessage(_ app: UnsafeMutableRawPointer, _ buffer: UnsafeMutablePointer<UInt8>, _ bufferLen: Int) -> Int

/// Public API for controlling a Bevy view
///
/// This provides a clean, Swift-friendly interface for interacting with Bevy
/// without exposing FFI or coordinator details.
class BevyViewController {
    private weak var coordinator: BevyMetalViewCoordinator?

    init(coordinator: BevyMetalViewCoordinator) {
        self.coordinator = coordinator
    }

    /// Send a message to Bevy
    func sendMessage(_ data: Data) {
        coordinator?.sendMessage(data)
    }

    /// Send a structured message to Bevy
    func send<T>(_ value: T) where T: Encodable {
        guard let data = try? JSONEncoder().encode(value) else { return }
        sendMessage(data)
    }

    /// Send raw bytes to Bevy
    func sendBytes(_ bytes: [UInt8]) {
        sendMessage(Data(bytes))
    }
}

/// A MetalKit view that hosts the Bevy engine
class BevyMetalViewCoordinator: NSObject, MTKViewDelegate {
    var bevyApp: UnsafeMutableRawPointer?
    var onMessageReceived: ((Data) -> Void)?

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        guard let app = bevyApp else { return }
        let scale = Float(size.width / view.bounds.width)
        bevyEmbeddedIosResize(app, UInt32(size.width), UInt32(size.height), scale)
    }

    func draw(in view: MTKView) {
        guard let app = bevyApp else { return }
        bevyEmbeddedUpdate(app)

        // Poll for messages from Bevy
        pollBevyMessages()
    }

    func pollBevyMessages() {
        guard let app = bevyApp, let callback = onMessageReceived else { return }

        var buffer = [UInt8](repeating: 0, count: 1024)
        let bytesRead = bevyEmbeddedIosReceiveMessage(app, &buffer, buffer.count)

        if bytesRead > 0 {
            let data = Data(buffer.prefix(bytesRead))
            DispatchQueue.main.async {
                callback(data)
            }
        }
    }

    func setupBevy(metalView: MTKView, size: CGSize, scale: CGFloat) {
        print("Setting up Bevy with size: \(size), scale: \(scale)")

        // Set the current surface for the callback
        let viewPtr = Unmanaged.passUnretained(metalView).toOpaque()
        currentSurface = EmbeddedSurfaceInfo(
            uiView: viewPtr,
            width: UInt32(size.width),
            height: UInt32(size.height),
            scaleFactor: Float(scale)
        )

        // Create the app - this will call bevy_embedded_get_surface() during plugin finish()
        bevyApp = bevyEmbeddedCreateApp()

        // Clear the surface info
        currentSurface = nil

        print("Bevy app initialized: \(bevyApp != nil)")
    }

    func handleTouch(phase: UInt8, location: CGPoint, id: UInt64) {
        guard let app = bevyApp else { return }
        bevyEmbeddedIosTouchEvent(app, phase, Float(location.x), Float(location.y), id)
    }

    func sendMessage(_ data: Data) {
        guard let app = bevyApp else { return }
        data.withUnsafeBytes { ptr in
            if let baseAddress = ptr.baseAddress {
                bevyEmbeddedIosSendMessage(app, baseAddress.assumingMemoryBound(to: UInt8.self), data.count)
            }
        }
    }

    deinit {
        if let app = bevyApp {
            bevyEmbeddedDestroy(app)
        }
    }
}

/// SwiftUI wrapper for the Bevy Metal view
struct BevyMetalView: UIViewRepresentable {
    typealias UIViewType = BevyTouchView

    /// Callback for messages received from Bevy
    var onMessageReceived: ((Data) -> Void)?

    /// Binding to control the view (send messages, etc.)
    @Binding var controller: BevyViewController?

    init(controller: Binding<BevyViewController?> = .constant(nil), onMessageReceived: ((Data) -> Void)? = nil) {
        self._controller = controller
        self.onMessageReceived = onMessageReceived
    }

    func makeCoordinator() -> BevyMetalViewCoordinator {
        let coord = BevyMetalViewCoordinator()
        coord.onMessageReceived = onMessageReceived
        DispatchQueue.main.async {
            // Expose a clean controller interface instead of raw coordinator
            self.controller = BevyViewController(coordinator: coord)
        }
        return coord
    }

    func makeUIView(context: Context) -> BevyTouchView {
        let touchView = BevyTouchView()
        touchView.coordinator = context.coordinator

        let metalView = MTKView()

        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }

        metalView.device = device
        metalView.preferredFramesPerSecond = 60
        metalView.enableSetNeedsDisplay = false
        metalView.isPaused = false  // MTKView drives the render loop
        metalView.framebufferOnly = true  // Optimize for rendering

        // Configure for embedded usage
        metalView.isMultipleTouchEnabled = true
        metalView.clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)

        // Add metalView as subview of touchView
        touchView.addSubview(metalView)
        metalView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            metalView.topAnchor.constraint(equalTo: touchView.topAnchor),
            metalView.bottomAnchor.constraint(equalTo: touchView.bottomAnchor),
            metalView.leadingAnchor.constraint(equalTo: touchView.leadingAnchor),
            metalView.trailingAnchor.constraint(equalTo: touchView.trailingAnchor)
        ])

        // Initialize Bevy after the view is configured
        DispatchQueue.main.async {
            let size = metalView.drawableSize
            // Get the scale from the drawable size vs bounds
            let scale = size.width / metalView.bounds.width
            context.coordinator.setupBevy(metalView: metalView, size: size, scale: scale)

            // Only set delegate after Bevy is initialized
            metalView.delegate = context.coordinator
        }

        return touchView
    }

    func updateUIView(_ uiView: BevyTouchView, context: Context) {
        // Handle any updates if needed
    }

    // Touch handling
    static func handleTouches(_ touches: Set<UITouch>, phase: UInt8, view: UIView, coordinator: BevyMetalViewCoordinator?) {
        guard let coord = coordinator else { return }

        for touch in touches {
            let location = touch.location(in: view)
            coord.handleTouch(phase: phase, location: location, id: UInt64(touch.hash))
        }
    }
}

/// A container view that captures touches for the Metal view
class BevyTouchView: UIView {
    var coordinator: BevyMetalViewCoordinator?

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        BevyMetalView.handleTouches(touches, phase: 0, view: self, coordinator: coordinator)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        BevyMetalView.handleTouches(touches, phase: 1, view: self, coordinator: coordinator)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        BevyMetalView.handleTouches(touches, phase: 2, view: self, coordinator: coordinator)
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        BevyMetalView.handleTouches(touches, phase: 3, view: self, coordinator: coordinator)
    }
}
