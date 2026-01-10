use super::multiplayer_menu::ColorButton;
use bevy::prelude::*;

/// Standard button styling
pub fn menu_button_bundle() -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            width: Val::Px(300.0),
            height: Val::Px(65.0),
            border: UiRect::all(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
        BorderColor::all(Color::BLACK),
    )
}

/// Standard button text styling
pub fn menu_button_text(text: impl Into<String>) -> (Text, TextFont, TextColor) {
    (
        Text::new(text),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
    )
}

/// System to handle button hover/press visual feedback
pub fn button_visual_feedback(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, Without<ColorButton>),
    >,
) {
    for (interaction, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                bg_color.0 = Color::srgb(0.35, 0.35, 0.35);
                *border_color = BorderColor::all(Color::srgb(0.8, 0.8, 0.8));
            }
            Interaction::Hovered => {
                bg_color.0 = Color::srgb(0.25, 0.25, 0.25);
                *border_color = BorderColor::all(Color::srgb(0.6, 0.6, 0.6));
            }
            Interaction::None => {
                bg_color.0 = Color::srgb(0.15, 0.15, 0.15);
                *border_color = BorderColor::all(Color::BLACK);
            }
        }
    }
}

/// Standard heading text styling
pub fn heading_text(text: impl Into<String>, font_size: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(text),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

/// Standard label text styling
pub fn label_text(text: impl Into<String>) -> (Text, TextFont, TextColor) {
    (
        Text::new(text),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
    )
}
