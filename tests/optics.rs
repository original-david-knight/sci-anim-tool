//! Ray optics: lenses, mirrors and media, and the rays and images traced through them, read
//! back from reports as an agent reads them.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn point(value: &Value) -> [f64; 2] {
    [value[0].as_f64().unwrap(), value[1].as_f64().unwrap()]
}

fn points(ray: &Value) -> Vec<[f64; 2]> {
    ray["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect()
}

fn near(a: [f64; 2], b: [f64; 2], within: f64) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < within
}

/// Where the line through a ray's last two points crosses `x`.
fn y_at(ray: &[[f64; 2]], x: f64) -> f64 {
    let (a, b) = (ray[ray.len() - 2], ray[ray.len() - 1]);
    a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0])
}

/// A lens of focal length 2 at `lens_x`, an object 1 tall standing on `foot` at `foot_x`, its
/// three principal rays and its image.
fn lens_scene(foot_x: f64, timeline: &str) -> String {
    let objects = format!(
        r#"
        "lens": {{ "type": "lens", "focal_length": 2, "height": 6 }},
        "foot": {{ "type": "dot", "radius": 0.05, "position": [{foot_x}, 0, 0] }},
        "parallel": {{ "type": "ray", "from": {{ "object": "foot", "offset": [0, 1, 0] }}, "principal": "parallel", "optics": ["lens"], "back_trace": true }},
        "center": {{ "type": "ray", "from": {{ "object": "foot", "offset": [0, 1, 0] }}, "principal": "center", "optics": ["lens"] }},
        "focal": {{ "type": "ray", "from": {{ "object": "foot", "offset": [0, 1, 0] }}, "principal": "focal", "optics": ["lens"] }},
        "image": {{ "type": "image", "from": {{ "object": "foot" }}, "to": {{ "object": "foot", "offset": [0, 1, 0] }}, "optics": ["lens"] }}"#
    );
    scene(&objects, timeline)
}

#[test]
fn a_converging_lens_images_by_the_thin_lens_equation_and_its_principal_rays_meet_there() {
    let stage = inspect_at(&lens_scene(-3.0, ""), "0");

    // 1/3 + 1/di = 1/2: di = 6, m = -di/do = -2.
    let image = &stage["objects"]["image"]["image"];
    assert!(near(point(&image["to"]), [6.0, -2.0], 1e-6), "{image}");
    assert!((image["magnification"].as_f64().unwrap() + 2.0).abs() < 1e-6);
    assert_eq!(image["real"], true);
    for ray in ["parallel", "center", "focal"] {
        let traced = points(&stage["objects"][ray]["ray"]);
        assert!((y_at(&traced, 6.0) + 2.0).abs() < 1e-6, "{ray}: {traced:?}");
    }
}

#[test]
fn inside_the_focal_length_the_image_is_virtual_and_the_back_trace_reaches_it() {
    let stage = inspect_at(&lens_scene(-1.0, ""), "0");

    // 1/1 + 1/di = 1/2: di = -2 on the object's side, m = 2.
    let image = &stage["objects"]["image"]["image"];
    assert!(near(point(&image["to"]), [-2.0, 2.0], 1e-6), "{image}");
    assert_eq!(image["real"], false);
    let parallel = &stage["objects"]["parallel"]["ray"];
    assert_eq!(parallel["virtual_image"], true);
    assert!(near(point(&parallel["image"]), [-2.0, 2.0], 1e-6));
}

#[test]
fn rays_and_images_follow_their_object_as_it_moves() {
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "foot", "to": [-4, 0, 0] } ] }"#;
    let scene = lens_scene(-3.0, timeline);

    let after = inspect_at(&scene, "1");

    // 1/4 + 1/di = 1/2: di = 4, m = -1.
    let image = &after["objects"]["image"]["image"];
    assert!(near(point(&image["to"]), [4.0, -1.0], 1e-6), "{image}");
    let center = points(&after["objects"]["center"]["ray"]);
    assert!(near(center[0], [-4.0, 1.0], 1e-6));
}

#[test]
fn a_concave_mirror_images_by_the_mirror_equation_and_reflects_its_rays_through_the_image() {
    let objects = r#"
        "mirror": { "type": "mirror", "focal_length": 2, "height": 3, "position": [3, 0, 0] },
        "foot": { "type": "dot", "radius": 0.05, "position": [-3, 0, 0] },
        "center": { "type": "ray", "from": { "object": "foot", "offset": [0, 0.5, 0] }, "principal": "center", "optics": ["mirror"] },
        "image": { "type": "image", "from": { "object": "foot" }, "to": { "object": "foot", "offset": [0, 0.5, 0] }, "optics": ["mirror"] }"#;

    let stage = inspect_at(&scene(objects, ""), "0");

    // do = 6: 1/6 + 1/di = 1/2 gives di = 3 in front of the mirror, at x 0, and m = -1/2.
    let image = &stage["objects"]["image"]["image"];
    assert!(near(point(&image["to"]), [0.0, -0.25], 1e-6), "{image}");
    assert_eq!(image["real"], true);
    let center = points(&stage["objects"]["center"]["ray"]);
    assert!((y_at(&center, 0.0) + 0.25).abs() < 1e-6, "{center:?}");
}

/// A glass slab of index 1.5 below y = 0, and a ray from `from` through the origin.
fn glass_ray(from: [f64; 2]) -> Vec<[f64; 2]> {
    let objects = format!(
        r#"
        "glass": {{ "type": "polygon", "points": [[-9, -4], [9, -4], [9, 0], [-9, 0]], "refractive_index": 1.5 }},
        "ray": {{ "type": "ray", "from": [{}, {}], "through": [0, 0], "optics": ["glass"], "length": 2 }}"#,
        from[0], from[1]
    );
    points(&inspect_at(&scene(&objects, ""), "0")["objects"]["ray"]["ray"])
}

#[test]
fn a_ray_refracts_into_glass_by_snell_s_law() {
    let traced = glass_ray([-2.0, 2.0]);

    // 45 degrees in air: sin 45° = 1.5 sin θ, so θ = 28.13° in the glass.
    let (bend, inside) = (traced[1], traced[2]);
    let angle = ((inside[0] - bend[0]) / (bend[1] - inside[1])).atan();
    assert!(
        (1.5 * angle.sin() - 45f64.to_radians().sin()).abs() < 1e-9,
        "{traced:?}"
    );
}

#[test]
fn past_the_critical_angle_a_ray_inside_glass_reflects_whole() {
    // From inside the glass at 41 and 43 degrees from the normal; the critical angle is 41.81.
    let ray = |degrees: f64| {
        let from = [-4.0 * degrees.to_radians().tan(), -4.0 + 1e-3];
        glass_ray(from)
    };

    let under = ray(41.0);
    let over = ray(43.0);

    assert!(under[2][1] > 0.0, "it leaves into the air: {under:?}");
    assert!(
        over[2][1] < 0.0,
        "it reflects back into the glass: {over:?}"
    );
}

#[test]
fn two_lenses_with_their_focal_points_together_magnify_angles_by_their_focal_lengths() {
    let objects = r#"
        "objective": { "type": "lens", "focal_length": 4, "height": 4, "position": [-3, 0, 0] },
        "eyepiece": { "type": "lens", "focal_length": 1, "height": 4, "position": [2, 0, 0] },
        "ray": { "type": "ray", "from": [-7, 0.6], "angle": -3, "optics": ["objective", "eyepiece"] }"#;

    let traced = points(&inspect_at(&scene(objects, ""), "0")["objects"]["ray"]["ray"]);

    let (a, b) = (traced[2], traced[3]);
    let slope = (b[1] - a[1]) / (b[0] - a[0]);
    assert!(
        (slope + 4.0 * (-3f64).to_radians().tan()).abs() < 1e-9,
        "{traced:?}"
    );
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn a_ray_names_its_optics_and_leaves_its_source_one_way() {
    let objects = r#"
        "lens": { "type": "lens", "focal_length": 2 },
        "glass": { "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "refractive_index": 1.5 },
        "plain": { "type": "circle", "radius": 1 },
        "no_aim": { "type": "ray", "from": [-3, 1], "optics": ["lens"] },
        "two_aims": { "type": "ray", "from": [-3, 1], "angle": 0, "principal": "center", "optics": ["lens"] },
        "not_optics": { "type": "ray", "from": [-3, 1], "angle": 0, "optics": ["plain", "nowhere", "lens", "lens"] },
        "no_lens": { "type": "ray", "from": [-3, 1], "principal": "focal", "optics": ["glass"] },
        "placed": { "type": "ray", "from": [-3, 1], "angle": 0, "optics": ["lens"], "position": [1, 0, 0] },
        "glass_image": { "type": "image", "from": [-3, 0], "to": [-3, 1], "optics": ["glass"] }"#;

    assert_eq!(
        errors(objects, ""),
        pairs(&[
            ("/objects/glass_image/optics/0", "invalid"),
            ("/objects/no_aim", "missing_field"),
            ("/objects/no_lens/principal", "invalid"),
            ("/objects/not_optics/optics/0", "invalid"),
            ("/objects/not_optics/optics/1", "unknown_object"),
            ("/objects/not_optics/optics/3", "duplicate_object"),
            ("/objects/placed/position", "placed_by_light"),
            ("/objects/two_aims", "invalid"),
        ])
    );
}

#[test]
fn a_ray_is_moved_by_its_source_and_its_lenses_not_itself() {
    let objects = r#"
        "lens": { "type": "lens", "focal_length": 2 },
        "ray": { "type": "ray", "from": [-3, 1], "angle": 0, "optics": ["lens"] }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "ray", "to": [1, 0, 0] } ] }"#;

    assert_eq!(
        errors(objects, timeline),
        pairs(&[("/timeline/0/animations/0/object", "placed_by_light")])
    );
}
