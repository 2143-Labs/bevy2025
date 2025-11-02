use bevy::app::{App, Plugin};

#[cfg(feature = "inspector")]
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "inspector")]
        {
            app.add_plugins((EguiPlugin::default(), WorldInspectorPlugin::new()));
        }

        #[cfg(not(feature = "inspector"))]
        {
            // Debug plugin is disabled - inspector feature not enabled
            let _ = app;
        }
    }
}