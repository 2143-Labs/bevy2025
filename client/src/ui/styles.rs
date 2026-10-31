use super::multiplayer_menu::ColorButton;
use bevy::prelude::*;
use bevy::{picking::hover::Hovered, ui::Pressed, ui_widgets::Button};

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
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
    )
}

/// System to handle button hover/press visual feedback
pub fn button_visual_feedback(
    mut interaction_query: Query<
        (
            &Hovered,
            Has<Pressed>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (With<Button>, Without<ColorButton>),
    >,
) {
    for (hovered, pressed, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        let (bg, border) = if pressed {
            (Color::srgb(0.35, 0.35, 0.35), Color::srgb(0.8, 0.8, 0.8))
        } else if hovered.get() {
            (Color::srgb(0.25, 0.25, 0.25), Color::srgb(0.6, 0.6, 0.6))
        } else {
            (Color::srgb(0.15, 0.15, 0.15), Color::BLACK)
        };
        if bg_color.0 != bg {
            bg_color.0 = bg;
            *border_color = BorderColor::all(border);
        }
    }
}

/// Standard heading text styling
pub fn heading_text(text: impl Into<String>, font_size: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(font_size),
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
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
    )
}
