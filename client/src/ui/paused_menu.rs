use super::styles::*;
use crate::game_state::GameState;
use bevy::prelude::*;

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct PausedMenu;

/// Marker for the Resume button
#[derive(Component)]
pub struct ResumeButton;

/// Marker for the Main Menu button
#[derive(Component)]
pub struct MainMenuButton;

/// Spawn the paused menu UI
pub fn spawn_paused_menu(mut commands: Commands) {
    info!("Spawning paused menu");
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(30.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), // Semi-transparent overlay
            PausedMenu,
        ))
        .with_children(|parent| {
            // "PAUSED" title
            parent.spawn({
                let (text, font, color) = heading_text("PAUSED", 80.0);
                (
                    Node {
                        margin: UiRect::bottom(Val::Px(40.0)),
                        ..default()
                    },
                    text,
                    font,
                    color,
                )
            });

            // Resume button
            {
                let (node, bg_color, border_color) = menu_button_bundle();
                let (text, font, color) = menu_button_text("Resume");
                parent
                    .spawn((
                        node,
                        bg_color,
                        border_color,
                        Interaction::default(),
                        ResumeButton,
                    ))
                    .with_children(|button| {
                        button.spawn((text, font, color));
                    });
            }

            // Main Menu button
            {
                let (node, bg_color, border_color) = menu_button_bundle();
                let (text, font, color) = menu_button_text("Main Menu");
                parent
                    .spawn((
                        node,
                        bg_color,
                        border_color,
                        Interaction::default(),
                        MainMenuButton,
                    ))
                    .with_children(|button| {
                        button.spawn((text, font, color));
                    });
            }
        });
}

/// Despawn the paused menu
pub fn despawn_paused_menu(mut commands: Commands, menu_query: Query<Entity, With<PausedMenu>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle button interactions in paused menu
pub fn handle_paused_menu_buttons(
    resume_query: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    main_menu_query: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<MainMenuButton>,
            Without<ResumeButton>,
        ),
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Resume button - go back to Playing
    for interaction in resume_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Resume button pressed");
            next_state.set(GameState::Playing);
        }
    }

    // Main Menu button - return to MainMenu
    for interaction in main_menu_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Main Menu button pressed from pause menu");
            next_state.set(GameState::MainMenu);
        }
    }
}
