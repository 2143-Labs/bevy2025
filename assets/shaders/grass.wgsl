#import bevy_pbr::{
    mesh_view_bindings::globals,
    mesh_view_bindings::view,
    forward_io::VertexOutput,
    mesh_functions,
    mesh_bindings::mesh,
    view_transformations::position_world_to_clip,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::apply_pbr_lighting,
    pbr_types::PbrInput,
}

struct GrassMaterial {
    wind_data: vec4<f32>, // direction.x, direction.y, strength, time
}

struct BallPositions {
    positions: array<vec4<f32>, 4>, // xyz = position, w = radius
}

struct BallCount {
    count: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> grass_material: GrassMaterial;

@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> ball_positions: BallPositions;

@group(#{MATERIAL_BIND_GROUP}) @binding(102)
var<uniform> ball_count: BallCount;

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    // Get wind parameters
    let wind_dir = vec2<f32>(grass_material.wind_data.x, grass_material.wind_data.y);
    let wind_strength = grass_material.wind_data.z;
    let time = globals.time;

    // Apply wind displacement in LOCAL space (before transform)
    // Wind wave calculation
    let gust = sin(time * 0.5 + position.x * 2.0 + position.z * 2.0) * 0.5 + 0.5;
    let ripple = sin(time * 2.0 + position.x * 5.0 + position.z * 5.0) * 0.3;
    let shimmer = sin(time * 4.0 + position.x * 10.0 + position.z * 10.0) * 0.2;

    let wind_wave = (gust + ripple + shimmer) * wind_strength;

    // Enhanced procedural blade curvature (Ghost of Tsushima technique)
    // Non-linear height factor for more natural curve (cubic for smooth S-curve)
    let height_factor = uv.y * uv.y * uv.y;

    // Combine wind with natural blade sway
    let bend_amount = wind_wave * height_factor;

    // Apply wind displacement in local space
    var bent_position = position;
    bent_position.x += wind_dir.x * bend_amount * 0.5;
    bent_position.z += wind_dir.y * bend_amount * 0.5;

    // Enhanced droop with height-based curve (more droop at tips)
    bent_position.y -= bend_amount * 0.15 + height_factor * 0.05;

    // Transform to world space manually
    var out: VertexOutput;
    var world_from_local = mesh_functions::get_world_from_local(instance_index);

    let world_position_temp = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(bent_position, 1.0));

    // Ball interaction - bend grass away from balls
    var ball_bend = vec3<f32>(0.0);
    for (var i = 0u; i < ball_count.count; i++) {
        let ball_pos = ball_positions.positions[i].xyz;
        let ball_radius = ball_positions.positions[i].w;

        // Calculate distance on XZ plane (horizontal distance)
        let to_grass = world_position_temp.xyz - ball_pos;
        let horizontal_dist = length(vec2<f32>(to_grass.x, to_grass.z));

        // Larger interaction radius for dense grass (more visible effect)
        let influence_radius = ball_radius * 4.0;

        if (horizontal_dist < influence_radius) {
            // Calculate influence strength (stronger when closer)
            let influence = 1.0 - (horizontal_dist / influence_radius);
            let influence_squared = influence * influence;
            let influence_cubed = influence_squared * influence; // Even stronger falloff near ball

            // Bend direction (away from ball on XZ plane)
            let bend_dir = normalize(vec2<f32>(to_grass.x, to_grass.z));

            // Stronger bending for dense grass visibility
            // Use cubic influence for very strong effect near ball, gentle far away
            let bend_strength = influence_cubed * height_factor * 1.5;
            ball_bend.x += bend_dir.x * bend_strength;
            ball_bend.z += bend_dir.y * bend_strength;

            // Stronger flattening effect
            ball_bend.y -= influence_cubed * height_factor * 0.6;
        }
    }

    // Apply ball interaction offset
    var world_position = vec4<f32>(world_position_temp.xyz + ball_bend, world_position_temp.w);

    // Camera-facing effects (Ghost of Tsushima technique)
    let camera_pos = view.world_position;
    let to_camera = normalize(camera_pos - world_position.xyz);
    let distance_to_camera = length(camera_pos - world_position.xyz);

    // Calculate blade orientation
    let blade_base = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
    let blade_forward = normalize(world_position.xyz - blade_base);

    // Camera alignment: 0 = perpendicular, 1 = facing camera
    let camera_alignment = abs(dot(to_camera, blade_forward));
    let perpendicular_factor = 1.0 - camera_alignment;

    // Billboard stretching for near/mid grass
    let stretch_factor = perpendicular_factor * height_factor * 1.5;
    let camera_right = normalize(cross(to_camera, vec3<f32>(0.0, 1.0, 0.0)));

    // View-space billboarding for distant grass (LOD technique)
    // Grass beyond 50m rotates to fully face camera
    let billboard_factor = smoothstep(50.0, 70.0, distance_to_camera) * height_factor;

    // Apply billboarding rotation (rotate blade to face camera)
    var final_position = world_position.xyz;

    // Stretch for perpendicular blades
    final_position += camera_right * stretch_factor * 0.15;

    // Billboard rotation for distant grass
    if (billboard_factor > 0.01) {
        // Rotate towards camera while preserving Y position
        let to_camera_xz = normalize(vec2<f32>(to_camera.x, to_camera.z));
        let offset_from_base = final_position - blade_base;

        // Rotate offset to face camera
        let rotated_offset = vec3<f32>(
            camera_right.x * offset_from_base.x,
            offset_from_base.y,
            camera_right.z * offset_from_base.x
        );

        final_position = mix(final_position, blade_base + rotated_offset, billboard_factor);
    }

    world_position = vec4<f32>(final_position, world_position.w);

    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);

    out.world_normal = mesh_functions::mesh_normal_local_to_world(normal, instance_index);
    out.uv = uv;

    return out;
}

// Fragment shader with proper PBR lighting and color variation
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    // Generate PBR input from standard material
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Procedural color variation based on world position
    let pos_hash = fract(sin(dot(in.world_position.xyz, vec3<f32>(12.9898, 78.233, 45.164))) * 43758.5453);

    // Base grass color with subtle variation
    let base_color = vec3<f32>(0.25, 0.6, 0.15);
    let color_variation = 0.85 + pos_hash * 0.3; // 0.85 to 1.15
    let green_boost = 0.95 + fract(pos_hash * 7.919) * 0.1;

    let varied_color = vec3<f32>(
        base_color.r * color_variation,
        base_color.g * color_variation * green_boost,
        base_color.b * color_variation
    );

    // Apply color variation to PBR input
    pbr_input.material.base_color = vec4<f32>(varied_color, 1.0);

    // Apply PBR lighting
    var output_color = apply_pbr_lighting(pbr_input);

    return output_color;
}
