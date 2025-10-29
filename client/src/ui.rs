use bevy::{
    color::palettes::basic::*,
    input_focus::InputFocus,
    prelude::*,
    image::{ImageSampler, ImageSamplerDescriptor},
};



pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<InputFocus>()
            .add_systems(Startup, setup)
            .add_systems(Update, (button_system, setup_logo_texture));
    }
}

/// Marker component for the main menu
#[derive(Component)]
pub struct MainMenu;

/// Marker components for different menu buttons
#[derive(Component)]
pub struct PlayButton;

#[derive(Component)]
pub struct JoinLobbyButton;

#[derive(Component)]
pub struct SettingsButton;

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Camera {
            is_active: true,
            order: 1,
            ..default()
        }
    ));

    // Main menu container
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: px(20),
                ..default()
            },
            MainMenu,
        ))
        .with_children(|parent| {
            // Logo - scaled to ~1/4 of screen height
            // Original size is 109x40, we'll scale it up significantly
            // Using nearest neighbor filtering for crisp pixel art
            parent.spawn((
                ImageNode {
                    image: assets.load("Logo.png"),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                Node {
                    width: px(436.0),  // 109 * 4 to maintain aspect ratio
                    height: px(160.0), // 40 * 4 to scale up
                    margin: UiRect::bottom(px(60)),
                    ..default()
                },
            ));

            // Play button
            parent.spawn(menu_button(&assets, "Play", PlayButton));

            // Join Lobby button
            parent.spawn(menu_button(&assets, "Join Lobby", JoinLobbyButton));

            // Settings button
            parent.spawn(menu_button(&assets, "Settings", SettingsButton));
        });
}

const NORMAL_BUTTON: Color = Color::srgb(0.2, 0.2, 0.25);
const HOVERED_BUTTON: Color = Color::srgb(0.3, 0.3, 0.4);
const PRESSED_BUTTON: Color = Color::srgb(0.4, 0.5, 0.7);
const NORMAL_BORDER: Color = Color::srgb(0.4, 0.4, 0.5);
const HOVERED_BORDER: Color = Color::srgb(0.6, 0.6, 0.8);
const PRESSED_BORDER: Color = Color::srgb(0.8, 0.85, 1.0);

fn button_system(
    mut input_focus: ResMut<InputFocus>,
    mut interaction_query: Query<
        (
            Entity,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Button,
        ),
        Changed<Interaction>,
    >,
) {
    for (entity, interaction, mut color, mut border_color, mut button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                input_focus.set(entity);
                *color = PRESSED_BUTTON.into();
                *border_color = BorderColor::all(PRESSED_BORDER);
                button.set_changed();
            }
            Interaction::Hovered => {
                input_focus.set(entity);
                *color = HOVERED_BUTTON.into();
                *border_color = BorderColor::all(HOVERED_BORDER);
                button.set_changed();
            }
            Interaction::None => {
                input_focus.clear();
                *color = NORMAL_BUTTON.into();
                *border_color = BorderColor::all(NORMAL_BORDER);
            }
        }
    }
}

/// System to apply nearest neighbor filtering to the logo texture
fn setup_logo_texture(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }

    let logo_handle: Handle<Image> = asset_server.load("Logo.png");

    if let Some(image) = images.get_mut(&logo_handle) {
        image.sampler = ImageSampler::nearest();
        *done = true;
    }
}

/// Creates a styled menu button with a marker component
fn menu_button(
    asset_server: &AssetServer,
    label: &str,
    marker: impl Component,
) -> impl Bundle {
    (
        Button,
        Node {
            width: px(280),
            height: px(70),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(NORMAL_BORDER),
        BorderRadius::all(px(8)),
        BackgroundColor(NORMAL_BUTTON),
        marker,
        children![(
            Text::new(label),
            TextFont {
                font: asset_server.load("fonts/PTSans-Regular.ttf"),
                font_size: 40.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.95, 0.95)),
        )],
    )
}