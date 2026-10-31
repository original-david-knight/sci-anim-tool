mod common;

use common::{frame_at, scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

fn scene(objects: Value) -> Value {
    json!({
        "scene_config": { "width": 161, "height": 121, "background": "transparent" },
        "objects": objects
    })
}

fn pixel(rgba: &[u8], x: usize, y: usize) -> &[u8] {
    &rgba[(y * 161 + x) * 4..(y * 161 + x + 1) * 4]
}

#[test]
fn only_background_accepts_transparent_and_schema_agrees_with_validate() {
    let printed = sci_anim(&["schema"]);
    let schema = jsonschema::validator_for(&printed.json).unwrap();
    for background in [
        "transparent",
        "#1e2F3a",
        "Transparent",
        "#ff000000",
        "none",
        "transparent\n",
    ] {
        let mut scene = scene(json!({}));
        scene["scene_config"]["background"] = json!(background);
        let expected = matches!(background, "transparent" | "#1e2F3a");
        let (_dir, path) = scene_file(&scene.to_string());
        let run = sci_anim(&["validate", path.to_str().unwrap()]);
        assert_eq!(schema.is_valid(&scene), expected, "{background:?}");
        assert_eq!(
            run.exit_code,
            if expected { 0 } else { 1 },
            "{}",
            run.stdout
        );
        if !expected {
            assert_eq!(scene_errors(&run)[0].0, "/scene_config/background");
        }
    }
    let scene = scene(
        json!({ "disc": { "type": "circle", "radius": 1, "fill": { "color": "transparent" } } }),
    );
    let (_dir, path) = scene_file(&scene.to_string());
    assert!(!schema.is_valid(&scene));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1);
    assert_eq!(scene_errors(&run)[0].0, "/objects/disc/fill/color");
}

#[test]
fn empty_transparent_pixels_are_zero_and_png_declares_srgb() {
    let (dir, rgba) = frame_at(&scene(json!({})).to_string(), "0");
    assert_eq!(rgba, vec![0; 161 * 121 * 4]);
    let reader = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(dir.path().join("f.png")).unwrap(),
    ))
    .read_info()
    .unwrap();
    assert_eq!((reader.info().width, reader.info().height), (161, 121));
    assert_eq!(
        reader.info().srgb,
        Some(png::SrgbRenderingIntent::Perceptual)
    );
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
}

#[test]
fn low_pixel_alpha_preserves_straight_color_and_zero_pixel_alpha_clears_rgb() {
    for (opacity, expected) in [
        (1.0 / 255.0, [0x26, 0x8c, 0xb5, 1]),
        (0.001, [0, 0, 0, 0]),
        (0.5, [0x26, 0x8c, 0xb5, 128]),
    ] {
        let scene = scene(
            json!({ "disc": { "type": "circle", "radius": 2, "opacity": opacity, "fill": { "color": "#268cb5" } } }),
        );
        let (_dir, rgba) = frame_at(&scene.to_string(), "0");
        assert_eq!(pixel(&rgba, 80, 60), expected, "opacity {opacity}");
        assert!(
            rgba.chunks_exact(4)
                .filter(|p| p[3] == 0)
                .all(|p| p == [0, 0, 0, 0])
        );
    }
}

#[test]
fn antialiased_pixels_keep_the_objects_color_at_partial_coverage() {
    let scene =
        scene(json!({ "disc": { "type": "circle", "radius": 2, "fill": { "color": "#268cb5" } } }));
    let (_dir, rgba) = frame_at(&scene.to_string(), "0");
    let partial_pixels: Vec<_> = rgba
        .chunks_exact(4)
        .filter(|p| p[3] > 0 && p[3] < 255)
        .collect();
    assert!(
        partial_pixels.len() > 50,
        "the circle has antialiased edge pixels"
    );
    for pixel in partial_pixels {
        assert_eq!(&pixel[..3], [0x26, 0x8c, 0xb5]);
        assert!(
            [64, 128, 191].contains(&pixel[3]),
            "four coverage samples give quarter steps: {pixel:?}"
        );
    }
}

#[test]
fn overlapping_faded_objects_and_material_opacity_accumulate_pixel_alpha() {
    let scene = scene(json!({
        "far": { "type": "circle", "radius": 2, "position": [0, 0, -1], "opacity": 0.5, "fill": { "color": "#ff0000" } },
        "near": { "type": "circle", "radius": 1, "opacity": 0.5, "fill": { "color": "#00ff00", "opacity": 0.5 } }
    }));
    let (_dir, rgba) = frame_at(&scene.to_string(), "0");
    let center = pixel(&rgba, 80, 60);
    assert_eq!(center[3], 159, "0.25 + 0.5 * 0.75 gives pixel alpha 0.625");
    assert!(
        center[0].abs_diff(203) <= 1 && center[1].abs_diff(170) <= 1 && center[2] == 0,
        "straight color is 60% red and 40% green in linear light: {center:?}"
    );
}

fn linear(byte: u8) -> f64 {
    let color = f64::from(byte) / 255.0;
    if color <= 0.04045 {
        color / 12.92
    } else {
        ((color + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn moving_fading_text_math_and_shapes_composite_over_black_and_white() {
    let mut scene = scene(json!({
        "words": { "type": "text", "text": "Alpha", "size": 1.1, "color": "#268cb5", "position": [0, 2.3, 0] },
        "math": { "type": "math", "tex": "x^2+1", "size": 1.1, "color": "#fc6255", "position": [0, 0, 0] },
        "disc": { "type": "circle", "radius": 0.8, "fill": { "color": "#51b360" }, "position": [-1, -2.2, 0] }
    }));
    scene["timeline"] = json!([{ "type": "play", "duration": 1, "animations": [
        { "type": "fade", "object": "words", "to": 0.2, "easing_curve": "linear" },
        { "type": "fade", "object": "math", "to": 0.2, "easing_curve": "linear" },
        { "type": "fade", "object": "disc", "to": 0.2, "easing_curve": "linear" },
        { "type": "move", "object": "words", "to": [0.6, 2.3, 0] },
        { "type": "move", "object": "math", "to": [-0.6, 0, 0] },
        { "type": "move", "object": "disc", "to": [1, -2.2, 0] }
    ] }]);
    for scene_time in ["0.25", "0.75"] {
        scene["scene_config"]["background"] = json!("transparent");
        let (_dir, transparent) = frame_at(&scene.to_string(), scene_time);
        assert!(transparent.chunks_exact(4).filter(|p| p[3] > 0).count() > 300);
        for (background, value) in [("#000000", 0.0), ("#ffffff", 1.0)] {
            scene["scene_config"]["background"] = json!(background);
            let (_dir, opaque) = frame_at(&scene.to_string(), scene_time);
            for (index, (pixel, expected)) in transparent
                .chunks_exact(4)
                .zip(opaque.chunks_exact(4))
                .enumerate()
            {
                let pixel_alpha = f64::from(pixel[3]) / 255.0;
                assert_eq!(expected[3], 255);
                for channel in 0..3 {
                    let composite =
                        linear(pixel[channel]) * pixel_alpha + value * (1.0 - pixel_alpha);
                    let error = (composite - linear(expected[channel])).abs();
                    // Both paths quantize sRGB; the opaque path also quantizes before its MSAA resolve.
                    // The transparent path adds at most half a byte of pixel alpha error.
                    assert!(
                        error <= 0.012,
                        "pixel {index}, channel {channel}, time {scene_time}, {background}: {pixel:?} over background vs {expected:?}, linear error {error}"
                    );
                }
            }
        }
    }
}
