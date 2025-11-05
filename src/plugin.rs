//! Embedded window plugin that replaces WinitPlugin for embedded mode

use bevy::{
    app::{App, Last, Plugin, PreUpdate},
    ecs::{
        entity::Entity,
        message::MessageWriter,
        query::With,
        system::{Query, ResMut},
    },
    input::touch::TouchInput,
    input::touch::TouchPhase as BevyTouchPhase,
    window::{Window, exit_on_all_closed},
};

use crate::{channel::*, input::*};

/// Plugin that provides embedded window support
///
/// This plugin replaces `WinitPlugin` when running Bevy in embedded mode.
/// Instead of creating windows through winit, the host application provides
/// window surfaces and forwards input events to Bevy.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_embedded::EmbeddedPlugin;
///
/// fn main() {
///     App::new()
///         .add_plugins((
///             MinimalPlugins,
///             EmbeddedPlugin,
///         ))
///         .run();
/// }
/// ```
#[derive(Default)]
pub struct EmbeddedPlugin;

impl Plugin for EmbeddedPlugin {
    fn name(&self) -> &str {
        "bevy_embedded::EmbeddedPlugin"
    }

    fn build(&self, app: &mut App) {
        app.init_resource::<EmbeddedInputEvents>()
            .init_resource::<HostChannel>()
            .add_systems(PreUpdate, process_embedded_input)
            .add_systems(Last, exit_on_all_closed);
    }

    fn finish(&self, app: &mut App) {
        // Verify WindowPlugin configuration after all plugins are added
        let windows = app
            .world_mut()
            .query::<&bevy::window::Window>()
            .iter(app.world())
            .count();

        if windows == 0 {
            panic!(
                "EmbeddedPlugin requires at least one window. \
                Make sure you've called create_window_from_host() before finishing the app."
            );
        }

        if windows > 1 {
            panic!(
                "EmbeddedPlugin found {} windows, expected 1. \
                Make sure WindowPlugin has primary_window set to None:\n\
                .set(WindowPlugin {{ primary_window: None, ..Default::default() }})",
                windows
            );
        }
    }
}

/// System that processes embedded input events and forwards them to Bevy's input systems
fn process_embedded_input(
    mut input_events: ResMut<EmbeddedInputEvents>,
    mut touch_writer: MessageWriter<TouchInput>,
    windows: Query<Entity, With<Window>>,
) {
    // Get the primary window entity (or first available)
    let window_entity = windows.iter().next();

    if let Some(window) = window_entity {
        // Process touch events
        for event in input_events.touch_events.drain(..) {
            let bevy_phase = match event.phase {
                TouchPhase::Started => BevyTouchPhase::Started,
                TouchPhase::Moved => BevyTouchPhase::Moved,
                TouchPhase::Ended => BevyTouchPhase::Ended,
                TouchPhase::Cancelled => BevyTouchPhase::Canceled,
            };

            touch_writer.write(TouchInput {
                phase: bevy_phase,
                position: event.position,
                window,
                force: None,
                id: event.id,
            });
        }
    }

    input_events.clear();
}
