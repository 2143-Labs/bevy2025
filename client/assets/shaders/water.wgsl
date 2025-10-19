#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings::globals,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct WaterMaterial {
    water_color: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> water_material: WaterMaterial;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Generate a PBR input from the standard material
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Animated water with waves using global time
    let time = globals.time;
    let uv = in.uv;

    // Create wave pattern
    let wave1 = sin(uv.x * 10.0 + time * 2.0) * 0.5 + 0.5;
    let wave2 = cos(uv.y * 10.0 + time * 1.5) * 0.5 + 0.5;
    let wave = (wave1 + wave2) * 0.5;

    // Base water color with wave influence
    let base_color = water_material.water_color;
    let final_color = vec4<f32>(
        base_color.r + wave * 0.1,
        base_color.g + wave * 0.15,
        base_color.b + wave * 0.2,
        base_color.a
    );

    // Override the base color with our animated water color
    pbr_input.material.base_color = final_color;

    // Alpha discard
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    // In deferred mode, lighting is run in a separate fullscreen shader
    let out = deferred_output(in, pbr_input);
#else
    // Apply PBR lighting in forward rendering mode
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
