//
//  ContentView.swift
//  test-bevy-embedded
//
//  Bevy Embedded Example
//

import SwiftUI

struct ContentView: View {
    @State private var bevyController: BevyViewController?
    @State private var cameraMatrix: String = "No camera data yet"

    var body: some View {
        ZStack {
            // Bevy render view
            BevyMetalView(
                controller: $bevyController,
                onMessageReceived: { data in
                    handleBevyMessage(data)
                }
            )
            .ignoresSafeArea()

            // UI overlay
            VStack {
                // Camera matrix display
                Text(cameraMatrix)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(.white)
                    .padding()
                    .background(Color.black.opacity(0.7))
                    .cornerRadius(8)
                    .padding()

                Spacer()

                HStack(spacing: 15) {
                    ColorButton(title: "Red", color: .red, controller: bevyController)
                    ColorButton(title: "Green", color: .green, controller: bevyController)
                    ColorButton(title: "Blue", color: .blue, controller: bevyController)
                    ColorButton(title: "Random", color: nil, controller: bevyController)
                }
                .padding()
                .background(Color.black.opacity(0.6))
                .cornerRadius(12)
                .padding()
            }
        }
    }

    func handleBevyMessage(_ data: Data) {
        if data.count == 64 {
            // Camera matrix (Mat4 = 16 floats = 64 bytes)
            let floats = data.withUnsafeBytes { ptr in
                Array(ptr.bindMemory(to: Float.self))
            }
            cameraMatrix = String(format: "Camera Mat4:\n[%.2f %.2f %.2f %.2f]\n[%.2f %.2f %.2f %.2f]\n[%.2f %.2f %.2f %.2f]\n[%.2f %.2f %.2f %.2f]",
                floats[0], floats[1], floats[2], floats[3],
                floats[4], floats[5], floats[6], floats[7],
                floats[8], floats[9], floats[10], floats[11],
                floats[12], floats[13], floats[14], floats[15])
        }
    }
}

struct ColorButton: View {
    let title: String
    let color: Color?
    let controller: BevyViewController?

    var body: some View {
        Button(title) {
            sendColor()
        }
        .foregroundColor(.white)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(color ?? Color.purple)
        .cornerRadius(8)
    }

    func sendColor() {
        var rgba: (Float, Float, Float, Float)

        if let color = color {
            let uiColor = UIColor(color)
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            uiColor.getRed(&r, green: &g, blue: &b, alpha: &a)
            rgba = (Float(r), Float(g), Float(b), Float(a))
        } else {
            // Random color
            rgba = (Float.random(in: 0...1), Float.random(in: 0...1), Float.random(in: 0...1), 1.0)
        }

        // Pack Vec4 as 16 bytes (4 x f32 in little-endian)
        var data = Data()
        withUnsafeBytes(of: rgba.0.bitPattern.littleEndian) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: rgba.1.bitPattern.littleEndian) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: rgba.2.bitPattern.littleEndian) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: rgba.3.bitPattern.littleEndian) { data.append(contentsOf: $0) }

        controller?.sendMessage(data)
    }
}

#Preview {
    ContentView()
}
