struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var vertices = array<vec3<f32>, 3>(
        vec3<f32>(0.0, 0.5, 0.0),
        vec3<f32>(-0.5, -0.5, 0.5),
        vec3<f32>(0.5, -0.5, 0.5),
    );

    var colors = array<vec4<f32>, 3>(
        vec4<f32>(1.0, 0.0, 1.0, 1.0),
        vec4<f32>(0.0, 1.0, 0.0, 1.0),
        vec4<f32>(0.0, 1.0, 1.0, 1.0),
    );

    let v = vertices[in_vertex_index];

    var out: VertexOutput;
    out.clip_position = vec4<f32>(v, 1.0);
    out.color = colors[in_vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
