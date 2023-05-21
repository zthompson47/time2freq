struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct Uniform {
    level: vec2<f32>,
    mouse_pos: vec2<f32>,
    screen_size: vec2<f32>,
    time: f32,
    loudness: f32,
}
@group(0) @binding(0)
var<uniform> u: Uniform;

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    let lvl_0 = 0.9 * u.level[0];
    let lvl_1 = 0.9 * u.level[1];
    var vertices = array<vec2<f32>, 8>(
        vec2<f32>(-0.5, lvl_0),
        vec2<f32>(-0.5, -0.9),
        vec2<f32>(-0.1, lvl_0),
        vec2<f32>(-0.1, -0.9),
        vec2<f32>(0.1, u.loudness), //lvl_1),
        vec2<f32>(0.1, -0.9),
        vec2<f32>(0.5, u.loudness), //lvl_1),
        vec2<f32>(0.5, -0.9),
    );

    var colors = array<vec4<f32>, 8>(
        vec4<f32>(0.0, 0.0, 1.0, 1.0),
        vec4<f32>(0.0, 1.0, 0.0, 1.0),
        vec4<f32>(1.0, 0.0, 0.0, 1.0),
        vec4<f32>(0.0, 0.0, 1.0, 1.0),
        vec4<f32>(1.0, 0.0, 0.0, 1.0),
        vec4<f32>(0.0, 1.0, 0.0, 1.0),
        vec4<f32>(0.0, 0.0, 1.0, 1.0),
        vec4<f32>(0.0, 0.0, 1.0, 1.0),
    );

    let v = vertices[in_vertex_index];

    var out: VertexOutput;
    out.clip_position = vec4<f32>(v, 0.0, 1.0);
    out.color = colors[in_vertex_index];
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let color = in.color;

    var mouse_fade =
        distance(in.clip_position.xy, u.mouse_pos) / max(u.screen_size.x, u.screen_size.y);

    var y_fade = f32(in.clip_position.y) / u.screen_size.y;

    //return color * pow(y_fade, 2.0) * mouse_fade;
    return color * pow(y_fade, 2.0);
    //return color;
}
