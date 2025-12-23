use bevy::{input::keyboard::KeyboardInput, prelude::*};

/// Text input component that handles user keyboard input
#[derive(Component, Debug, Clone)]
pub struct TextInput {
    pub value: String,
    pub placeholder: String,
    pub max_length: usize,
    pub is_focused: bool,
}

impl TextInput {
    pub fn new(placeholder: impl Into<String>, max_length: usize) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            max_length,
            is_focused: false,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }
}

/// Marker for the text display entity (child of text input)
#[derive(Component)]
pub struct TextInputDisplay;

/// Resource to track which input field is currently focused
#[derive(Resource, Default)]
pub struct FocusedInput(pub Option<Entity>);

/// System to handle clicking on text input fields
pub fn handle_text_input_focus(
    interaction_query: Query<
        (Entity, &Interaction, &Children),
        (Changed<Interaction>, With<TextInput>),
    >,
    mut all_inputs: Query<&mut TextInput>,
    mut text_query: Query<&mut TextColor, With<TextInputDisplay>>,
    mut focused: ResMut<FocusedInput>,
) {
    for (entity, interaction, children) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            // Unfocus previous input
            if let Some(prev_entity) = focused.0 {
                if let Ok(mut prev_input) = all_inputs.get_mut(prev_entity) {
                    prev_input.is_focused = false;
                }
            }

            // Focus this input
            if let Ok(mut input) = all_inputs.get_mut(entity) {
                input.is_focused = true;
            }
            focused.0 = Some(entity);

            // Update text color to show focus
            for child in children.iter() {
                if let Ok(mut text_color) = text_query.get_mut(child) {
                    text_color.0 = Color::WHITE;
                }
            }
        }
    }
}

/// System to handle keyboard input for focused text field
pub fn handle_text_input_keyboard(
    mut keyboard_events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_query: Query<(&mut TextInput, &Children)>,
    mut text_query: Query<&mut Text, With<TextInputDisplay>>,
    focused: Res<FocusedInput>,
) {
    // Only process if we have a focused input
    let Some(focused_entity) = focused.0 else {
        return;
    };

    let Ok((mut input, children)) = input_query.get_mut(focused_entity) else {
        return;
    };

    let mut changed = false;

    // Handle backspace
    if keyboard.just_pressed(KeyCode::Backspace) {
        if !input.value.is_empty() {
            input.value.pop();
            changed = true;
        }
    }

    // Handle keyboard events for text input
    for event in keyboard_events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        // Handle printable characters
        if let Some(text_char) = keycode_to_char(
            event.key_code,
            keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        ) {
            if input.value.len() < input.max_length {
                input.value.push(text_char);
                changed = true;
            }
        }
    }

    // Update display text if changed
    if changed {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                if input.value.is_empty() {
                    text.0 = input.placeholder.clone();
                } else {
                    text.0 = input.value.clone();
                }
            }
        }
    }
}

/// Convert KeyCode to character (simplified version)
fn keycode_to_char(key: KeyCode, shift: bool) -> Option<char> {
    match key {
        // Numbers
        KeyCode::Digit0 => Some(if shift { ')' } else { '0' }),
        KeyCode::Digit1 => Some(if shift { '!' } else { '1' }),
        KeyCode::Digit2 => Some(if shift { '@' } else { '2' }),
        KeyCode::Digit3 => Some(if shift { '#' } else { '3' }),
        KeyCode::Digit4 => Some(if shift { '$' } else { '4' }),
        KeyCode::Digit5 => Some(if shift { '%' } else { '5' }),
        KeyCode::Digit6 => Some(if shift { '^' } else { '6' }),
        KeyCode::Digit7 => Some(if shift { '&' } else { '7' }),
        KeyCode::Digit8 => Some(if shift { '*' } else { '8' }),
        KeyCode::Digit9 => Some(if shift { '(' } else { '9' }),

        // Letters
        KeyCode::KeyA => Some(if shift { 'A' } else { 'a' }),
        KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
        KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
        KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
        KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
        KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
        KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
        KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
        KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
        KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
        KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
        KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
        KeyCode::KeyM => Some(if shift { 'M' } else { 'm' }),
        KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
        KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
        KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
        KeyCode::KeyQ => Some(if shift { 'Q' } else { 'q' }),
        KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
        KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
        KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
        KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
        KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
        KeyCode::KeyW => Some(if shift { 'W' } else { 'w' }),
        KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
        KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
        KeyCode::KeyZ => Some(if shift { 'Z' } else { 'z' }),

        // Special characters
        KeyCode::Space => Some(' '),
        KeyCode::Period => Some(if shift { '>' } else { '.' }),
        KeyCode::Comma => Some(if shift { '<' } else { ',' }),
        KeyCode::Minus => Some(if shift { '_' } else { '-' }),
        KeyCode::Equal => Some(if shift { '+' } else { '=' }),
        KeyCode::Slash => Some(if shift { '?' } else { '/' }),
        KeyCode::Semicolon => Some(if shift { ':' } else { ';' }),
        KeyCode::Quote => Some(if shift { '"' } else { '\'' }),
        KeyCode::BracketLeft => Some(if shift { '{' } else { '[' }),
        KeyCode::BracketRight => Some(if shift { '}' } else { ']' }),
        KeyCode::Backslash => Some(if shift { '|' } else { '\\' }),
        KeyCode::Backquote => Some(if shift { '~' } else { '`' }),

        _ => None,
    }
}

/// System to update visual feedback for focused/unfocused inputs
pub fn update_text_input_visual_feedback(
    mut input_query: Query<(&TextInput, &mut BorderColor), Changed<TextInput>>,
) {
    for (input, mut border_color) in input_query.iter_mut() {
        if input.is_focused {
            *border_color = BorderColor::all(Color::srgb(0.2, 0.6, 1.0)); // Blue border when focused
        } else {
            *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4)); // Gray border when unfocused
        }
    }
}
