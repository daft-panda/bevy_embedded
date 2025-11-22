//! Embedded Bevy example for iOS

use bevy::{
    input::touch::{TouchInput, TouchPhase},
    log::{Level, LogPlugin},
    prelude::*,
};
use bevy_embedded::{export_embedded_app, prelude::*};

/// Our embedded app implementation
struct MobileEmbeddedExample;

impl EmbeddedApp for MobileEmbeddedExample {
    fn setup(app: &mut App) {
        // Configure plugins and systems
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::winit::WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None, // We create our own window
                    ..Default::default()
                })
                .set(LogPlugin {
                    level: Level::DEBUG,
                    filter: "wgpu=debug,bevy_render=debug,bevy_ecs=debug".to_string(),
                    ..Default::default()
                }),
        )
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (touch_camera, handle_messages));
    }
}

// Export the FFI entry points
export_embedded_app!(MobileEmbeddedExample);

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(5.0, 5.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.1, 0.2, 0.1))),
    ));

    // cube (this one will change color from Swift messages)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(Color::srgb(0.5, 0.4, 0.3))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        ColorChangingCube,
    ));

    // sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(4).unwrap())),
        MeshMaterial3d(materials.add(Color::srgb(0.1, 0.4, 0.8))),
        Transform::from_xyz(1.5, 1.5, 1.5),
    ));

    // light
    commands.spawn((
        PointLight {
            intensity: 1_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn touch_camera(
    mut touch_inputs: MessageReader<TouchInput>,
    mut camera_transform: Single<&mut Transform, With<Camera3d>>,
    mut last_position: Local<Option<Vec2>>,
    windows: Query<&Window>,
    channel: Res<HostChannel>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    for touch_input in touch_inputs.read() {
        if touch_input.phase == TouchPhase::Started {
            *last_position = None;
        }
        if let Some(last_pos) = *last_position {
            **camera_transform = Transform::from_xyz(
                camera_transform.translation.x
                    + (touch_input.position.x - last_pos.x) / window.width() * 5.0,
                camera_transform.translation.y,
                camera_transform.translation.z
                    + (touch_input.position.y - last_pos.y) / window.height() * 5.0,
            )
            .looking_at(Vec3::ZERO, Vec3::Y);

            // Send camera transform matrix to host app
            let mat = camera_transform.to_matrix();
            let bytes: [u8; 64] = bytemuck::cast(mat.to_cols_array());
            channel.send(bytes.to_vec());
        }
        *last_position = Some(touch_input.position);
    }
}

/// Component to mark the cube that changes color
#[derive(Component)]
struct ColorChangingCube;

fn handle_messages(
    channel: Res<HostChannel>,
    mut cubes: Query<&mut MeshMaterial3d<StandardMaterial>, With<ColorChangingCube>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Receive Vec4 color messages from Swift (16 bytes: 4 f32s for RGBA)
    while let Some(message) = channel.receive() {
        if message.len() == 16 {
            // Unpack 4 f32s from the binary blob
            let r = f32::from_le_bytes([message[0], message[1], message[2], message[3]]);
            let g = f32::from_le_bytes([message[4], message[5], message[6], message[7]]);
            let b = f32::from_le_bytes([message[8], message[9], message[10], message[11]]);
            let a = f32::from_le_bytes([message[12], message[13], message[14], message[15]]);

            info!("Received color from Swift: ({}, {}, {}, {})", r, g, b, a);

            // Update cube material color
            for material_handle in cubes.iter_mut() {
                if let Some(material) = materials.get_mut(&material_handle.0) {
                    material.base_color = Color::srgba(r, g, b, a);
                }
            }
        } else {
            warn!(
                "Received message with unexpected length: {} bytes",
                message.len()
            );
        }
    }
}
