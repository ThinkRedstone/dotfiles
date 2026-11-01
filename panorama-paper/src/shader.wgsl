struct Camera {
    // Quaternion rotating camera space into world space.
    orientation: vec4<f32>,
    half_extent: vec2<f32>,
    offset: vec2<f32>,
}

@group(0) @binding(0) var panorama: texture_cube<f32>;
@group(0) @binding(1) var panorama_sampler: sampler;
@group(0) @binding(2) var<uniform> camera: Camera;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

// A single triangle covering the whole screen.
@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let ndc = vec2(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return VertexOutput(vec4(ndc, 0.0, 1.0), ndc);
}

@fragment
fn fragment_main(@location(0) ndc: vec2<f32>) -> @location(0) vec4<f32> {
    let surface_position = camera.offset + ndc * camera.half_extent;
    var direction = normalize(vec3(surface_position, -1.0));
    let rotation_axis = camera.orientation.xyz;
    let doubled_cross = 2.0 * cross(rotation_axis, direction);
    direction += camera.orientation.w * doubled_cross + cross(rotation_axis, doubled_cross);
    // Cubemap lookups are left-handed; flipping z keeps the faces unmirrored.
    return textureSample(panorama, panorama_sampler, vec3(direction.x, direction.y, -direction.z));
}
