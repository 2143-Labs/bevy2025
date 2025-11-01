use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, VertexAttributeValues},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        experimental::occlusion_culling::OcclusionCulling,
        render_resource::{AsBindGroup, PrimitiveTopology},
    },
    shader::ShaderRef,
};
use noise::{NoiseFn, Perlin};
use shared::net_components::ents::Ball;

use crate::{camera::LocalCamera, network::DespawnOnWorldData};

/// Marker for grass entities
#[derive(Component)]
pub struct Grass;

/// Grass chunk for LOD management
#[derive(Component)]
pub struct GrassChunk {
    #[allow(dead_code)]
    pub center: Vec3,
    #[allow(dead_code)]
    pub size: f32,
}

/// Wind settings resource
#[derive(Resource)]
pub struct WindSettings {
    pub direction: Vec2,
    pub strength: f32,
    pub time: f32,
}

impl Default for WindSettings {
    fn default() -> Self {
        Self {
            direction: Vec2::new(1.0, 0.5).normalize(),
            strength: 0.5,
            time: 0.0,
        }
    }
}

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindSettings>()
            .add_plugins(MaterialPlugin::<
                ExtendedMaterial<StandardMaterial, GrassMaterial>,
            >::default())
            .add_systems(Update, (update_wind_time, update_ball_interactions));
    }
}

/// Custom grass material extension
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct GrassMaterial {
    /// Wind data: direction.x, direction.y, strength, time
    #[uniform(100)]
    pub wind_data: Vec4,

    /// Ball positions for interaction (up to 8 balls, w component is radius)
    #[uniform(101)]
    pub ball_positions: [Vec4; 8],

    /// Number of active balls
    #[uniform(102)]
    pub ball_count: u32,
}

impl MaterialExtension for GrassMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }
}

/// Update wind time for animation
fn update_wind_time(mut wind: ResMut<WindSettings>, time: Res<Time>) {
    wind.time += time.delta_secs();
}

/// Timer resource for throttling ball interaction updates
#[derive(Resource)]
struct BallInteractionTimer {
    timer: Timer,
}

impl Default for BallInteractionTimer {
    fn default() -> Self {
        Self {
            // Update ball positions at 30 FPS instead of every frame
            timer: Timer::from_seconds(1.0 / 30.0, TimerMode::Repeating),
        }
    }
}

/// Update ball positions in all grass materials for interaction
fn update_ball_interactions(
    time: Res<Time>,
    mut update_timer: Local<Option<BallInteractionTimer>>,
    all_balls: Query<&Transform, With<Ball>>,
    // TODO make this change if you change camera?
    camera_query: Query<&Transform, (With<Camera>, With<LocalCamera>)>,
    mut grass_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, GrassMaterial>>>,
) {
    // Initialize timer on first run
    let timer = update_timer.get_or_insert_with(BallInteractionTimer::default);

    // Only update at fixed intervals (30 FPS)
    timer.timer.tick(time.delta());
    if !timer.timer.just_finished() {
        return;
    }

    // Get camera position to prioritize nearest balls
    let camera_pos = camera_query
        .iter()
        .find_map(|t| Some(t.translation))
        .unwrap_or(Vec3::ZERO);

    // Collect balls sorted by distance to camera (prioritize nearest 8)
    let mut ball_data: Vec<(f32, Vec4)> = all_balls
        .iter()
        .map(|transform| {
            let dist_to_camera = camera_pos.distance(transform.translation);
            let ball_vec = Vec4::new(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                0.5, // Ball radius
            );
            (dist_to_camera, ball_vec)
        })
        .collect();

    // Sort by distance (nearest first)
    ball_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Take closest 8 balls
    let mut ball_positions = [Vec4::ZERO; 8];
    let mut count = 0u32;

    for (i, (_, ball_vec)) in ball_data.iter().take(8).enumerate() {
        ball_positions[i] = *ball_vec;
        count += 1;
    }

    // Update all grass materials with ball data (now only 1 shared material!)
    for (_, material) in grass_materials.iter_mut() {
        material.extension.ball_positions = ball_positions;
        material.extension.ball_count = count;
    }
}

/// Create a grass blade mesh using a bezier curve
/// Returns a quad mesh that curves naturally
pub fn create_grass_blade_mesh() -> Mesh {
    let height = 1.0;
    let width = 0.1;
    let segments = 4; // Number of vertical segments for the curve

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    // Create bezier curve for natural blade shape
    // Control points: base (0), mid-low (slight curve), mid-high (more curve), tip (max curve)
    let p0 = Vec3::new(0.0, 0.0, 0.0); // Base
    let p1 = Vec3::new(0.1, height * 0.33, 0.0); // Lower control
    let p2 = Vec3::new(0.2, height * 0.66, 0.0); // Upper control
    let p3 = Vec3::new(0.3, height, 0.0); // Tip

    // Generate vertices along the bezier curve
    for i in 0..=segments {
        let t = i as f32 / segments as f32;

        // Cubic bezier curve formula
        let one_minus_t = 1.0 - t;
        let curve_point = p0 * one_minus_t.powi(3)
            + p1 * 3.0 * one_minus_t.powi(2) * t
            + p2 * 3.0 * one_minus_t * t.powi(2)
            + p3 * t.powi(3);

        let y = curve_point.y;
        let forward_bend = curve_point.x;

        // Width tapers towards the tip
        let segment_width = width * (1.0 - t * 0.7);

        // Left and right vertices
        positions.push([forward_bend - segment_width / 2.0, y, 0.0]);
        positions.push([forward_bend + segment_width / 2.0, y, 0.0]);

        // Normals point forward
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);

        // UVs
        uvs.push([0.0, t]);
        uvs.push([1.0, t]);

        // Base color (will be modulated per-blade in merged mesh)
        colors.push([1.0, 1.0, 1.0, 1.0]);
        colors.push([1.0, 1.0, 1.0, 1.0]);
    }

    // Generate indices for triangles
    for i in 0..segments {
        let base = (i * 2) as u32;

        // First triangle
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 1);

        // Second triangle
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base + 3);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Spawn grass instances on the terrain
pub fn spawn_grass_on_terrain(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    _materials: &mut ResMut<Assets<StandardMaterial>>,
    grass_materials: &mut ResMut<Assets<ExtendedMaterial<StandardMaterial, GrassMaterial>>>,
    wind: &WindSettings,
    terrain_size: f32,
    grass_density: f32,
    terrain_seed: u32,
    terrain_height_scale: f32,
    water_level: f32,
) {
    let grass_mesh = meshes.add(create_grass_blade_mesh());
    let noise = Perlin::new(terrain_seed);

    // Create a SINGLE shared grass material for all blades
    let grass_material = grass_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.25, 0.6, 0.15), // Natural grass green
            alpha_mode: AlphaMode::Opaque,
            cull_mode: None, // Render both sides
            unlit: false,    // Use lighting for better visuals
            ..default()
        },
        extension: GrassMaterial {
            wind_data: Vec4::new(wind.direction.x, wind.direction.y, wind.strength, wind.time),
            ball_positions: [Vec4::ZERO; 8],
            ball_count: 0,
        },
    });

    // Chunk configuration for LOD
    const CHUNK_SIZE: f32 = 20.0;
    let chunks_per_side = (terrain_size / CHUNK_SIZE).ceil() as i32;

    // LOD configuration - distance-based density
    const LOD_NEAR_DIST: f32 = 30.0; // Full density within 30m
    const LOD_MID_DIST: f32 = 60.0; // Medium density 30-60m
    const LOD_FAR_DIST: f32 = 80.0; // Low density 60-80m
    // Beyond 80m: no grass (culled)

    // Spawn grass organized into chunks
    for chunk_x in 0..chunks_per_side {
        for chunk_z in 0..chunks_per_side {
            // Calculate chunk bounds
            let chunk_min_x = -terrain_size / 2.0 + chunk_x as f32 * CHUNK_SIZE;
            let chunk_min_z = -terrain_size / 2.0 + chunk_z as f32 * CHUNK_SIZE;
            let chunk_center = Vec3::new(
                chunk_min_x + CHUNK_SIZE / 2.0,
                0.0,
                chunk_min_z + CHUNK_SIZE / 2.0,
            );

            // Calculate distance from origin (camera spawn point) for LOD
            let dist_from_origin =
                (chunk_center.x * chunk_center.x + chunk_center.z * chunk_center.z).sqrt();

            // Smooth LOD transitions with gradual falloff
            let density_multiplier = if dist_from_origin > LOD_FAR_DIST {
                continue; // Skip chunks beyond far distance
            } else if dist_from_origin > LOD_MID_DIST {
                // Smooth transition between mid and far (60m to 80m)
                let t = (dist_from_origin - LOD_MID_DIST) / (LOD_FAR_DIST - LOD_MID_DIST);
                0.6 - (0.3 * t) // Fade from 60% to 30%
            } else if dist_from_origin > LOD_NEAR_DIST {
                // Smooth transition between near and mid (30m to 60m)
                let t = (dist_from_origin - LOD_NEAR_DIST) / (LOD_MID_DIST - LOD_NEAR_DIST);
                1.0 - (0.4 * t) // Fade from 100% to 60%
            } else {
                1.0 // Full density for near chunks
            };

            // Apply LOD density multiplier
            let chunk_grass_density = grass_density * density_multiplier;

            // Collect all grass blade transforms for this chunk
            let mut chunk_blade_data = Vec::new();

            // Spawn grass blades within this chunk at full density grid
            // We'll thin it out randomly to match the target density
            let blades_per_chunk_side = (CHUNK_SIZE * grass_density) as i32; // Always use max density grid
            let spacing = CHUNK_SIZE / blades_per_chunk_side as f32;

            for x in 0..blades_per_chunk_side {
                for z in 0..blades_per_chunk_side {
                    // Random culling to achieve target density (smooths LOD transitions)
                    if fastrand::f32() > density_multiplier {
                        continue; // Skip this blade based on LOD
                    }
                    // Calculate position relative to chunk bounds
                    let base_x = chunk_min_x + x as f32 * spacing;
                    let base_z = chunk_min_z + z as f32 * spacing;

                    // Add random jitter
                    let jitter_x = (fastrand::f32() - 0.5) * spacing * 0.8;
                    let jitter_z = (fastrand::f32() - 0.5) * spacing * 0.8;

                    let pos_x = base_x + jitter_x;
                    let pos_z = base_z + jitter_z;

                    // Sample terrain height using same noise function as terrain
                    let noise_x = pos_x * 0.05; // Same scale as terrain
                    let noise_z = pos_z * 0.05;
                    let terrain_height =
                        noise.get([noise_x as f64, noise_z as f64]) as f32 * terrain_height_scale;

                    // Skip grass below water level
                    if terrain_height < water_level {
                        continue;
                    }

                    // Height-based density: sparse at peaks, dense at mid-elevations
                    // Calculate normalized height (0.0 = water level, 1.0 = max height)
                    let height_range = terrain_height_scale - water_level;
                    let normalized_height = (terrain_height - water_level) / height_range;

                    // Density falloff at peaks (90%+ height)
                    let peak_threshold = 0.9;
                    if normalized_height > peak_threshold {
                        // No grass at very top, increasingly sparse as we approach peak
                        let peak_factor =
                            (normalized_height - peak_threshold) / (1.0 - peak_threshold);
                        if fastrand::f32() < peak_factor * 0.95 {
                            // Skip this blade (95% chance to skip at max height)
                            continue;
                        }
                    }

                    // Additional random thinning at high elevations (70-90%)
                    if normalized_height > 0.7 {
                        let high_factor = (normalized_height - 0.7) / 0.2;
                        if fastrand::f32() < high_factor * 0.3 {
                            continue;
                        }
                    }

                    // Random rotation
                    let rotation = fastrand::f32() * std::f32::consts::TAU;

                    // Random scale variation
                    let scale = 0.8 + fastrand::f32() * 0.4; // 0.8 to 1.2

                    // Random color variation (subtle green variations)
                    let color_var = 0.85 + fastrand::f32() * 0.3; // 0.85 to 1.15
                    let green_boost = 0.95 + fastrand::f32() * 0.1; // Slightly more green
                    let color = Vec3::new(
                        0.25 * color_var,              // Red channel
                        0.6 * color_var * green_boost, // Green channel (boosted)
                        0.15 * color_var,              // Blue channel
                    );

                    // Position grass at terrain height
                    let position = Vec3::new(pos_x, terrain_height, pos_z);

                    // Store blade data for merging (position, rotation, scale, color)
                    chunk_blade_data.push((position, rotation, scale, color));
                }
            }

            // Merge all grass blades in this chunk into a single mesh
            if !chunk_blade_data.is_empty() {
                let merged_mesh = create_merged_grass_mesh(&grass_mesh, meshes, &chunk_blade_data);
                let merged_mesh_handle = meshes.add(merged_mesh);

                // Spawn single chunk entity with merged mesh
                commands.spawn((
                    GrassChunk {
                        center: chunk_center,
                        size: CHUNK_SIZE,
                    },
                    Grass,
                    Mesh3d(merged_mesh_handle),
                    MeshMaterial3d(grass_material.clone()),
                    Transform::default(),
                    Visibility::default(),
                    OcclusionCulling, // Enable Bevy's built-in occlusion culling
                    DespawnOnWorldData,
                ));
            }
        }
    }
}

/// Create a merged mesh from multiple grass blade instances
fn create_merged_grass_mesh(
    base_mesh_handle: &Handle<Mesh>,
    meshes: &Assets<Mesh>,
    blade_data: &[(Vec3, f32, f32, Vec3)], // (position, rotation, scale, color)
) -> Mesh {
    let base_mesh = meshes.get(base_mesh_handle).unwrap();

    // Extract base mesh data
    let base_positions = base_mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let base_normals = base_mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    let base_uvs: &[[f32; 2]] = match base_mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
        VertexAttributeValues::Float32x2(uvs) => uvs,
        _ => panic!("Expected Float32x2 for UVs"),
    };
    let base_indices = match base_mesh.indices().unwrap() {
        Indices::U32(indices) => indices,
        _ => panic!("Expected U32 indices"),
    };

    let verts_per_blade = base_positions.len();
    let indices_per_blade = base_indices.len();

    // Allocate space for merged mesh
    let mut merged_positions = Vec::with_capacity(blade_data.len() * verts_per_blade);
    let mut merged_normals = Vec::with_capacity(blade_data.len() * verts_per_blade);
    let mut merged_uvs = Vec::with_capacity(blade_data.len() * verts_per_blade);
    let mut merged_colors = Vec::with_capacity(blade_data.len() * verts_per_blade);
    let mut merged_indices = Vec::with_capacity(blade_data.len() * indices_per_blade);

    // Merge each blade
    for (blade_idx, &(position, rotation, scale, color)) in blade_data.iter().enumerate() {
        let vertex_offset = (blade_idx * verts_per_blade) as u32;

        // Transform and add vertices
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();

        for i in 0..verts_per_blade {
            let base_pos = base_positions[i];
            // Apply scale, rotation (around Y), and translation
            let scaled = [
                base_pos[0] * scale,
                base_pos[1] * scale,
                base_pos[2] * scale,
            ];
            let rotated = [
                scaled[0] * cos_r - scaled[2] * sin_r,
                scaled[1],
                scaled[0] * sin_r + scaled[2] * cos_r,
            ];
            let final_pos = [
                rotated[0] + position.x,
                rotated[1] + position.y,
                rotated[2] + position.z,
            ];
            merged_positions.push(final_pos);

            // Rotate normals
            let base_norm = base_normals[i];
            let rotated_norm = [
                base_norm[0] * cos_r - base_norm[2] * sin_r,
                base_norm[1],
                base_norm[0] * sin_r + base_norm[2] * cos_r,
            ];
            merged_normals.push(rotated_norm);

            // Copy UVs as-is
            merged_uvs.push(base_uvs[i]);

            // Apply per-blade color variation
            merged_colors.push([color.x, color.y, color.z, 1.0]);
        }

        // Add indices with offset
        for &index in base_indices {
            merged_indices.push(index + vertex_offset);
        }
    }

    // Create merged mesh
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, merged_positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, merged_normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, merged_uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, merged_colors)
    .with_inserted_indices(Indices::U32(merged_indices))
}
