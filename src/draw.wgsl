struct Uniforms {
    world_to_clip: mat4x4<f32>,
    object_to_world: mat4x4<f32>,
    normal_to_world: mat4x4<f32>,
    color: vec4<f32>,
    camera_direction: vec3<f32>,
    lighting: u32,
    key_light: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct PointIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct PointOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn point_main(point: PointIn) -> PointOut {
    var out: PointOut;
    out.clip = uniforms.world_to_clip * uniforms.object_to_world * vec4(point.position, 1.0);
    out.normal = (uniforms.normal_to_world * vec4(point.normal, 0.0)).xyz;
    return out;
}

// `lighting` is `mesh::Lighting`.
const LIGHTING_CAMERA: u32 = 0u;
const LIGHTING_KEY: u32 = 1u;
const LIGHTING_SHEET: u32 = 3u;

// A light along the camera's axis, with ambient and diffuse summing to 1, so a surface
// facing the camera shows exactly its color. Either side of a flat shape counts as facing.
const AMBIENT: f32 = 0.25;

// The key light, from the upper left: a face toward it shows exactly its color, and a face away
// from it and the camera still the ambient. The shade then steps through `KEY_BANDS` tones with
// soft joins, so a curved solid reads as drawn in a few tones rather than as smooth plastic.
const KEY_AMBIENT: f32 = 0.22;
const KEY_DIFFUSE: f32 = 0.68;
const KEY_FILL: f32 = 0.2;
const KEY_BANDS: f32 = 3.0;

@fragment
fn surface_main(in: PointOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var normal = normalize(in.normal);
    var shade = 1.0;
    // A sheet's triangles turn counter-clockwise toward the side its normals point to, so where
    // the camera sees the other side, that side is lit as its own.
    if uniforms.lighting == LIGHTING_SHEET && !front_facing {
        normal = -normal;
    }
    if uniforms.lighting == LIGHTING_CAMERA {
        shade = AMBIENT + (1.0 - AMBIENT) * abs(dot(normal, uniforms.camera_direction));
    } else if uniforms.lighting == LIGHTING_KEY || uniforms.lighting == LIGHTING_SHEET {
        let key = max(dot(normal, uniforms.key_light), 0.0);
        let fill = max(dot(normal, uniforms.camera_direction), 0.0);
        let lit = min(KEY_AMBIENT + KEY_DIFFUSE * key + KEY_FILL * fill, 1.0) * KEY_BANDS;
        shade = (floor(lit) + smoothstep(0.3, 0.7, fract(lit))) / KEY_BANDS;
    }
    return vec4(uniforms.color.rgb * shade, uniforms.color.a);
}
