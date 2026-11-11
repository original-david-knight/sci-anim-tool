//! Vector fields: the electric field of charges and the magnetic field of wires, loops and
//! magnets, read back from `inspect` and `validate` as an agent reads them.

mod common;

use std::f64::consts::PI;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn point(value: &Value) -> [f64; 3] {
    [0, 1, 2].map(|i| value[i].as_f64().unwrap())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn field<'a>(stage: &'a Value, id: &str) -> &'a Value {
    &stage["objects"][id]["vector_field"]
}

fn lines(stage: &Value, id: &str) -> Vec<Value> {
    field(stage, id)["lines"].as_array().unwrap().clone()
}

/// Whether a world point of the plane z = 0 lies on the edge of the rectangle `half` from the
/// origin.
fn on_edge(p: [f64; 3], half: [f64; 2]) -> bool {
    (p[0].abs() - half[0]).abs() < 1e-9 || (p[1].abs() - half[1]).abs() < 1e-9
}

/// A charge of +1 at (-2, 0) and one of `minus_charge` at (2, 0), both circles of radius 0.25,
/// with `per_charge` lines a unit of charge in a rectangle 12 by 8.
fn dipole(minus_charge: f64, per_charge: u32, timeline: &str) -> String {
    let objects = format!(
        r#"
        "plus": {{ "type": "circle", "radius": 0.25, "position": [-2, 0, 0] }},
        "minus": {{ "type": "circle", "radius": 0.25, "position": [2, 0, 0] }},
        "E": {{ "type": "vector-field", "of": "electric",
          "sources": [ {{ "type": "charge", "object": "plus", "charge": 1 }}, {{ "type": "charge", "object": "minus", "charge": {minus_charge} }} ],
          "draw": {{ "type": "lines", "width": 12, "height": 8, "per_charge": {per_charge} }} }}"#
    );
    scene(&objects, timeline)
}

#[test]
fn a_dipole_s_lines_leave_the_plus_and_end_on_the_minus_or_the_edge_alike_above_and_below() {
    let stage = inspect_at(&dipole(-1.0, 12, ""), "0");
    let lines = lines(&stage, "E");

    let from_plus: Vec<&Value> = lines
        .iter()
        .filter(|line| (distance(point(&line["from"]), [-2.0, 0.0, 0.0]) - 0.25).abs() < 1e-9)
        .collect();
    assert_eq!(from_plus.len(), 12, "{lines:?}");
    for line in &from_plus {
        let to = point(&line["to"]);
        let on_minus =
            line["ends_on"] == "minus" && (distance(to, [2.0, 0.0, 0.0]) - 0.25).abs() < 1e-9;
        assert!(on_minus || on_edge(to, [6.0, 4.0]), "{line}");
    }
    assert!(from_plus.iter().any(|line| line["ends_on"] == "minus"));
    // The minus takes as many lines as the plus gives, so it starts those that reach it from
    // the edge itself: 12 end on it in all.
    let on_minus = lines.iter().filter(|l| l["ends_on"] == "minus").count();
    assert_eq!(on_minus, 12, "{lines:?}");
    // The scene is symmetric about the x axis, and so are its lines.
    for line in &lines {
        let [x, y, _] = point(&line["to"]);
        let mirrored = lines
            .iter()
            .any(|other| distance(point(&other["to"]), [x, -y, 0.0]) < 1e-6);
        assert!(mirrored, "no line ends at ({x}, {}): {lines:?}", -y);
    }
}

#[test]
fn a_lone_charge_s_lines_run_straight_out_at_even_angles() {
    let objects = r#"
        "q": { "type": "dot", "radius": 0.1, "position": [1, 0.5, 0] },
        "E": { "type": "vector-field", "of": "electric",
          "sources": [ { "type": "charge", "object": "q", "charge": 2 } ],
          "draw": { "type": "lines", "width": 10, "height": 10, "per_charge": 4 },
          "position": [1, 0.5, 0] }"#;

    let stage = inspect_at(&scene(objects, ""), "0");
    let lines = lines(&stage, "E");

    // 4 lines a unit of charge on a charge of 2.
    assert_eq!(lines.len(), 8);
    for (k, line) in lines.iter().enumerate() {
        let [fx, fy, _] = point(&line["from"]);
        let [tx, ty, _] = point(&line["to"]);
        let expected = 2.0 * PI * k as f64 / 8.0;
        let start = (fy - 0.5).atan2(fx - 1.0).rem_euclid(2.0 * PI);
        let end = (ty - 0.5).atan2(tx - 1.0).rem_euclid(2.0 * PI);
        assert!(
            (start - expected).abs() < 1e-9,
            "line {k} leaves at {start}"
        );
        assert!((end - expected).abs() < 1e-6, "line {k} bends to {end}");
        assert!(on_edge([tx - 1.0, ty - 0.5, 0.0], [5.0, 5.0]), "{line}");
    }
}

/// A wire through the origin seen end on, carrying 1 toward the camera, as a circle of 0.1.
const WIRE: &str = r#""w": { "type": "circle", "radius": 0.1 }"#;

#[test]
fn a_wire_s_lines_are_circles_about_it_and_its_field_is_2_i_over_rho() {
    let objects = format!(
        r##"{WIRE},
        "B": {{ "type": "vector-field", "of": "magnetic",
          "sources": [ {{ "type": "wire", "object": "w", "current": 1 }} ],
          "draw": {{ "type": "lines", "width": 8, "height": 8, "seeds": [[1.5, 0]], "tip_length": 0.01 }},
          "stroke": {{ "color": "#ffffff", "width": 0.01 }} }}"##
    );

    let stage = inspect_at(&scene(&objects, ""), "0");
    let lines = lines(&stage, "B");

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["closed"], true);
    assert!(distance(point(&lines[0]["from"]), [1.5, 0.0, 0.0]) < 1e-9);
    // A circle of radius 1.5 fills the box from -1.5 to 1.5 each way, its stroke 0.01 wide.
    let world = &stage["objects"]["B"]["bounding_box"]["world"];
    for (corner, sign) in [("min", -1.0), ("max", 1.0)] {
        for axis in 0..2 {
            let reach = world[corner][axis].as_f64().unwrap() * sign;
            assert!((reach - 1.505).abs() < 2e-3, "{corner}: {world}");
        }
    }
    // Counterclockwise seen from the end the current flows toward: up at +x, 2I/ρ = 4/3.
    let at = point(&field(&stage, "B")["at_seeds"][0]);
    assert!(distance(at, [0.0, 4.0 / 3.0, 0.0]) < 1e-12, "{at:?}");
}

#[test]
fn arrows_around_a_wire_point_around_it_and_none_is_drawn_inside_it() {
    let objects = format!(
        r#"{WIRE},
        "B": {{ "type": "vector-field", "of": "magnetic",
          "sources": [ {{ "type": "wire", "object": "w", "current": 1 }} ],
          "draw": {{ "type": "arrows", "layout": {{ "type": "grid", "counts": [3, 3, 1], "spacing": 1 }} }} }}"#
    );

    let stage = inspect_at(&scene(&objects, ""), "0");
    let arrows = field(&stage, "B")["arrows"].as_array().unwrap().clone();

    assert_eq!(
        field(&stage, "B")["arrow_count"],
        8,
        "the middle place is inside the wire"
    );
    let arrow_at = |place: [f64; 3]| {
        arrows
            .iter()
            .find(|a| distance(point(&a["place"]), place) < 1e-9)
            .unwrap_or_else(|| panic!("an arrow at {place:?}: {arrows:?}"))
    };
    for (place, toward, strength) in [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 2.0),
        ([0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], 2.0),
        ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], 2.0),
        (
            [1.0, 1.0, 0.0],
            [-0.5f64.sqrt(), 0.5f64.sqrt(), 0.0],
            2.0f64.sqrt(),
        ),
    ] {
        let arrow = arrow_at(place);
        assert!(distance(point(&arrow["toward"]), toward) < 1e-9, "{arrow}");
        assert!(
            (arrow["strength"].as_f64().unwrap() - strength).abs() < 1e-9,
            "{arrow}"
        );
    }
}

#[test]
fn arrows_in_a_grid_in_space_point_along_the_field_in_space() {
    let objects = r#"
        "q": { "type": "sphere", "radius": 0.2 },
        "E": { "type": "vector-field", "of": "electric",
          "sources": [ { "type": "charge", "object": "q", "charge": 1 } ],
          "draw": { "type": "arrows", "layout": { "type": "grid", "counts": [3, 3, 3], "spacing": 1 } } }"#;

    let stage = inspect_at(&scene(objects, ""), "0");
    let arrows = field(&stage, "E")["arrows"].as_array().unwrap().clone();

    assert_eq!(arrows.len(), 26);
    let corner = arrows
        .iter()
        .find(|a| distance(point(&a["place"]), [1.0, 1.0, 1.0]) < 1e-9)
        .unwrap();
    let out = 1.0 / 3f64.sqrt();
    assert!(
        distance(point(&corner["toward"]), [out, out, out]) < 1e-9,
        "{corner}"
    );
    // q/r² at r² = 3.
    assert!((corner["strength"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-9);
}

#[test]
fn a_magnet_s_lines_run_through_it_and_close() {
    let objects = r#"
        "bar": { "type": "polygon", "points": [[-1, -0.3], [1, -0.3], [1, 0.3], [-1, 0.3]] },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "magnet", "object": "bar", "length": 2, "radius": 0.3 } ],
          "draw": { "type": "lines", "width": 12, "height": 8, "count": 3 } }"#;

    let stage = inspect_at(&scene(objects, ""), "0");
    let lines = lines(&stage, "B");

    assert_eq!(lines.len(), 6);
    for line in &lines {
        assert_eq!(line["closed"], true, "{line}");
        assert_eq!(line["from"], line["to"]);
        // Each starts on the line through the magnet's middle, outside it, above or below.
        let [x, y, _] = point(&line["from"]);
        assert!(x.abs() < 1e-9 && y.abs() > 0.3, "{line}");
    }
    // Inside a magnet its field runs south to north, toward +x.
    let seeded = r#"
        "bar": { "type": "polygon", "points": [[-1, -0.3], [1, -0.3], [1, 0.3], [-1, 0.3]] },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "magnet", "object": "bar", "length": 2, "radius": 0.3 } ],
          "draw": { "type": "lines", "width": 12, "height": 8, "seeds": [[0, 0], [0, 2]] } }"#;
    let at = field(&inspect_at(&scene(seeded, ""), "0"), "B")["at_seeds"].clone();
    assert!(
        at[0][0].as_f64().unwrap() > 0.0 && at[1][0].as_f64().unwrap() < 0.0,
        "{at}"
    );
}

#[test]
fn a_loop_s_field_on_its_axis_is_the_textbook_s() {
    // The loop's own z is turned onto the world's x, so its axis lies in the field's plane.
    let objects = r#"
        "ring": { "type": "circle", "radius": 1.5, "rotation": [0, 90, 0] },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "loop", "object": "ring", "radius": 1.5, "current": 2 } ],
          "draw": { "type": "lines", "width": 12, "height": 8, "seeds": [[0, 0], [1, 0], [3, 0], [0, 1]] } }"#;

    let stage = inspect_at(&scene(objects, ""), "0");
    let at = field(&stage, "B")["at_seeds"].as_array().unwrap().clone();

    // μ0 I a² / (2 (a² + z²)^(3/2)) with μ0 = 4π.
    for (seed, z) in at.iter().zip([0.0, 1.0, 3.0]) {
        let expected = 2.0 * PI * 2.0 * 1.5f64.powi(2) / (1.5f64.powi(2) + z * z).powf(1.5);
        let [bx, by, bz] = point(seed);
        assert!(
            (bx - expected).abs() < 1e-9 * expected,
            "at z {z}: {bx} {expected}"
        );
        assert!(by.abs() < 1e-9 && bz.abs() < 1e-9);
    }
    // A line through it off its axis closes around its wire, which crosses the plane at y 1.5.
    assert_eq!(lines(&stage, "B")[3]["closed"], true);
}

#[test]
fn moving_a_charge_moves_the_lines_that_end_on_it() {
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "minus", "to": [2, 1.5, 0] } ] }"#;
    let scene = dipole(-1.0, 8, timeline);

    for (scene_time, at) in [("0", [2.0, 0.0, 0.0]), ("1", [2.0, 1.5, 0.0])] {
        let stage = inspect_at(&scene, scene_time);
        let ending: Vec<Value> = lines(&stage, "E")
            .into_iter()
            .filter(|line| line["ends_on"] == "minus")
            .collect();
        assert_eq!(ending.len(), 8);
        for line in ending {
            let to = point(&line["to"]);
            assert!(
                (distance(to, at) - 0.25).abs() < 1e-9,
                "at {scene_time}: {line}"
            );
        }
    }
}

#[test]
fn turning_a_magnet_turns_its_field() {
    let objects = r#"
        "bar": { "type": "polygon", "points": [[-1, -0.3], [1, -0.3], [1, 0.3], [-1, 0.3]] },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "magnet", "object": "bar", "length": 2, "radius": 0.3 } ],
          "draw": { "type": "arrows", "layout": { "type": "grid", "counts": [1, 1, 1], "spacing": 1 } },
          "position": [0, 3, 0] }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "rotate", "object": "bar", "to": [0, 0, 180] } ] }"#;
    let scene = scene(objects, timeline);

    let toward = |scene_time| {
        let stage = inspect_at(&scene, scene_time);
        point(&field(&stage, "B")["arrows"][0]["toward"])
    };

    // Beside a magnet's middle its field runs back from north to south.
    assert!(toward("0")[0] < -0.99);
    assert!(toward("1")[0] > 0.99);
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn a_vector_field_names_field_sources_of_its_own_field_on_objects_with_a_place() {
    let objects = r#"
        "plus": { "type": "circle", "radius": 0.25 },
        "lens": { "type": "lens", "focal_length": 2 },
        "ray": { "type": "ray", "from": [-3, 1], "angle": 0, "optics": ["lens"] },
        "E": { "type": "vector-field", "of": "electric", "scale": [2, 2, 1],
          "sources": [
            { "type": "charge", "object": "nowhere", "charge": 1 },
            { "type": "loop", "object": "plus", "radius": 1, "current": 1 },
            { "type": "charge", "object": "ray", "charge": 1 },
            { "type": "charge", "object": "E", "charge": 1 }
          ],
          "draw": { "type": "lines", "width": 4, "height": 3 } },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "wire", "object": "plus", "current": 1, "direction": [0, 0, 0] } ],
          "draw": { "type": "arrows", "layout": { "type": "grid", "counts": [3, 3, 1], "spacing": 1 } } }"#;

    assert_eq!(
        errors(objects, ""),
        pairs(&[
            ("/objects/B/sources/0/direction", "invalid"),
            ("/objects/E/sources/0/object", "unknown_object"),
            ("/objects/E/sources/1/type", "invalid"),
            ("/objects/E/sources/2/object", "invalid"),
            ("/objects/E/sources/3/object", "invalid"),
            ("/objects/E/scale", "invalid"),
        ])
    );
}

#[test]
fn a_vector_field_draws_lines_of_its_own_field_in_a_rectangle_with_room() {
    let objects = r#"
        "plus": { "type": "circle", "radius": 0.25 },
        "none": { "type": "vector-field", "of": "electric", "sources": [],
          "draw": { "type": "lines", "width": 4, "height": 0 } }"#;
    assert_eq!(
        errors(objects, ""),
        pairs(&[
            ("/objects/none/draw/height", "out_of_range"),
            ("/objects/none/sources", "wrong_length"),
        ])
    );

    let objects = r#"
        "plus": { "type": "circle", "radius": 0.25 },
        "E": { "type": "vector-field", "of": "electric",
          "sources": [ { "type": "charge", "object": "plus", "charge": 1 } ],
          "draw": { "type": "lines", "width": 4, "height": 3, "count": 3 } },
        "B": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "wire", "object": "plus", "current": 1 } ],
          "draw": { "type": "lines", "width": 4, "height": 3, "per_charge": 3 } },
        "S": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "wire", "object": "plus", "current": 1 } ],
          "draw": { "type": "lines", "width": 4, "height": 3, "count": 3, "seeds": [[1, 0]] } },
        "A": { "type": "vector-field", "of": "magnetic",
          "sources": [ { "type": "wire", "object": "plus", "current": 1 } ],
          "draw": { "type": "arrows", "layout": { "type": "grid", "counts": [100, 100, 1], "spacing": 0.1 } } }"#;
    assert_eq!(
        errors(objects, ""),
        pairs(&[
            ("/objects/A/draw/layout/counts", "out_of_range"),
            ("/objects/B/draw/per_charge", "invalid"),
            ("/objects/E/draw/count", "invalid"),
            ("/objects/S/draw/count", "invalid"),
        ])
    );
}

#[test]
fn a_vector_field_is_faded_created_and_recolored_but_not_scaled_or_morphed() {
    let objects = r#"
        "plus": { "type": "circle", "radius": 0.25 },
        "E": { "type": "vector-field", "of": "electric",
          "sources": [ { "type": "charge", "object": "plus", "charge": 1 } ],
          "draw": { "type": "lines", "width": 4, "height": 3 } }"#;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [
        { "type": "create", "object": "E" },
        { "type": "fade", "object": "E", "to": 0.5 },
        { "type": "recolor", "object": "E", "stroke": "#ff0000" },
        { "type": "recolor", "object": "E", "fill": "#ff0000" },
        { "type": "scale", "object": "E", "to": [2, 2, 1] },
        { "type": "morph", "object": "E", "into": "plus" } ] }"##;

    assert_eq!(
        errors(objects, timeline),
        pairs(&[
            ("/timeline/0/animations/3/fill", "not_recolorable"),
            ("/timeline/0/animations/5/object", "not_morphable"),
            ("/timeline/0/animations/4/object", "invalid"),
        ])
    );
}

#[test]
fn a_create_draws_each_line_out_from_its_start() {
    let objects = r##"
        "q": { "type": "dot", "radius": 0.1 },
        "E": { "type": "vector-field", "of": "electric",
          "sources": [ { "type": "charge", "object": "q", "charge": 1 } ],
          "draw": { "type": "lines", "width": 8, "height": 8, "per_charge": 4 },
          "stroke": { "color": "#ffffff", "width": 0.12 } }"##;
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "create", "object": "E" } ] }"#;
    let scene = scene(objects, timeline);
    // About 48 pixels a world unit, the line along +x on row 200: world x 0.5 and 3.5.
    let (near, far) = ((224, 200), (369, 200));
    let lit = |rgba: &[u8], (x, y): (usize, usize)| pixel(rgba, x, y)[0] > 128;

    let (_dir, early) = frame_at(&scene, "0.5");
    let (_dir, whole) = frame_at(&scene, "2");

    assert!(lit(&early, near) && !lit(&early, far));
    assert!(lit(&whole, near) && lit(&whole, far));
}
