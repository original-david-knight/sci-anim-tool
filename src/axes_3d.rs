//! 3D axes: where data `(x, y, z)` lands in the object's own space, with z up as a textbook draws
//! it, and the axis lines, ticks and labels that show it.
//!
//! Data `(x, y, z)` lands at `(x, z, -y)` in the object's own space, each axis scaled so its range
//! fills its length in a box centered on the object's position: x to the right, z up and y away
//! from the default camera, the turn a prism's base takes. The lines and ticks are drawn as unlit
//! sticks a stroke wide, so they keep their width from every side. Each label faces the camera at
//! every frame, and a z tick and its label lie across the frame, so `mesh` places them from the
//! camera through `Facing`, and `inspect` measures them the same way.

use glam::{Mat3, Mat4, Vec2, Vec3};
use kurbo::{Affine, BezPath};
use serde::Serialize;

use crate::axes::{
    Appearance, Axis, AxisReport, LABEL_GAP, LabelError, Labelled, Side, Spot, TICK_LENGTH, Tick,
    TickLabel,
};
use crate::mesh::{BoundingBox, corners};
use crate::path::Path;
use crate::scene::{Align, Camera, Glyph, Stroke};
use crate::typeset::{self, Layout, TypesetError};

/// The three axes, by the data coordinate each shows.
const NAMES: [&str; 3] = ["x", "y", "z"];

#[derive(Clone, Debug)]
pub struct Axes3d {
    /// `x`, `y` and `z`, in that order.
    pub axes: [Axis; 3],
    /// The three axis lines, then every tick: the order a create draws them in.
    rulings: Vec<Ruling>,
    pub stroke: Stroke,
    pub labels: Vec<Label3d>,
}

/// An axis line or a tick, in the object's own space.
#[derive(Clone, Copy, Debug)]
enum Ruling {
    /// An axis line, or an x or y tick, which stands up along z.
    Fixed([Vec3; 2]),
    /// A z tick about this point, which lies across the frame, so it shows from every side.
    AcrossFrame(Vec3),
}

/// A tick label or an axis label of 3D axes: its glyphs, centered on the origin of a plane that
/// faces the camera, and where its center sits.
#[derive(Clone, Debug)]
pub struct Label3d {
    pub of: Labelled,
    pub text: String,
    /// Which side of its tick a tick label sits on. An axis label sits past the high end of its
    /// axis and has none.
    pub side: Option<Side>,
    pub glyphs: Vec<Glyph>,
    /// Half the width and height of the box around the glyphs.
    pub half: Vec2,
    place: Place,
}

/// Where a label's center sits.
#[derive(Clone, Copy, Debug)]
enum Place {
    /// At this point of the object's own space.
    At(Vec3),
    /// Left of a z tick at this point of the object's own space, along the frame, with the tick's
    /// half length and the label gap between them.
    LeftOf(Vec3),
}

/// How the camera sees 3D axes at one scene time: the object's transform, the frame's right and
/// up, the direction toward the camera, and how much the object's scale grows a label, the cube
/// root of how much it grows a volume.
#[derive(Clone, Copy)]
pub struct Facing {
    object_to_world: Mat4,
    right: Vec3,
    up: Vec3,
    toward_camera: Vec3,
    size: f32,
}

impl Facing {
    pub fn new(object_to_world: Mat4, camera: &Camera) -> Facing {
        let forward = (camera.target - camera.position).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = right.cross(forward);
        Facing {
            object_to_world,
            right,
            up,
            toward_camera: -forward,
            size: Mat3::from_mat4(object_to_world).determinant().abs().cbrt(),
        }
    }

    fn world(&self, own: Vec3) -> Vec3 {
        self.object_to_world.transform_point3(own)
    }
}

impl Axes3d {
    /// `labels` are LaTeX math, set past the high end of the x, y and z axis.
    pub fn new(
        axes: [Axis; 3],
        labels: [Option<&str>; 3],
        appearance: Appearance,
    ) -> Result<Axes3d, LabelError> {
        let crossing = axes.map(|axis| axis.crossing());
        let own = |data: [f64; 3]| DataToOwn::of(&axes).own(data);
        let with = |k: usize, value: f64| {
            let mut data = crossing;
            data[k] = value;
            own(data)
        };
        let mut rulings: Vec<Ruling> = (0..3)
            .map(|k| Ruling::Fixed([with(k, axes[k].range().0), with(k, axes[k].range().1)]))
            .collect();
        let mut labels_built = Vec::new();
        // Typesets `tex` and centers it where `place` puts a label of its half width and height.
        let label = |of: Labelled,
                     tex: &str,
                     side: Option<Side>,
                     place: &dyn Fn(Vec2) -> Place|
         -> Result<Label3d, LabelError> {
            let (Labelled::Tick { axis, .. } | Labelled::Axis { axis, .. }) = of;
            let (glyphs, half) =
                centered(tex, appearance).map_err(|error| LabelError { axis, error })?;
            Ok(Label3d {
                place: place(half),
                of,
                text: tex.to_string(),
                side,
                glyphs,
                half,
            })
        };
        // An x or y tick label sits below its axis, or above one that runs over the box's middle.
        let (away, x_and_y_side) = if own(crossing).y > 0.0 {
            (Vec3::Y, Side::Above)
        } else {
            (-Vec3::Y, Side::Below)
        };
        for (k, axis) in axes.iter().enumerate() {
            for value in axis.ticks() {
                let at = with(k, value);
                rulings.push(if k == 2 {
                    Ruling::AcrossFrame(at)
                } else {
                    Ruling::Fixed([
                        at - Vec3::Y * TICK_LENGTH / 2.0,
                        at + Vec3::Y * TICK_LENGTH / 2.0,
                    ])
                });
                // The tick where the axes cross would set its label on the other axes.
                if value == crossing[k] {
                    continue;
                }
                let (place, side): (&dyn Fn(Vec2) -> Place, Side) = match k {
                    2 => (&|_| Place::LeftOf(at), Side::Left),
                    _ => (
                        &|half: Vec2| {
                            Place::At(at + away * (TICK_LENGTH / 2.0 + LABEL_GAP + half.y))
                        },
                        x_and_y_side,
                    ),
                };
                labels_built.push(label(
                    Labelled::Tick {
                        axis: NAMES[k],
                        value,
                    },
                    &axis.label(value),
                    Some(side),
                    place,
                )?);
            }
        }
        for (k, tex) in labels.iter().enumerate() {
            let Some(tex) = tex else { continue };
            let (low, end) = (with(k, axes[k].range().0), with(k, axes[k].range().1));
            // Past the end by its half width along x, its half height along z, and the larger
            // along y, which runs toward or away from the camera.
            let past = |half: Vec2| {
                Place::At(
                    end + (end - low).normalize()
                        * (LABEL_GAP + [half.x, half.x.max(half.y), half.y][k]),
                )
            };
            labels_built.push(label(
                Labelled::Axis {
                    axis: NAMES[k],
                    label: tex.to_string(),
                },
                tex,
                None,
                &past,
            )?);
        }
        Ok(Axes3d {
            axes,
            rulings,
            stroke: appearance.stroke,
            labels: labels_built,
        })
    }

    /// Where a data point lands in the object's own space.
    pub fn to_own(&self, data: [f64; 3]) -> Vec3 {
        self.data_to_own().own(data)
    }

    pub fn data_to_own(&self) -> DataToOwn {
        DataToOwn::of(&self.axes)
    }

    /// The box the axes' ranges span, in the object's own space, low corner first: what a
    /// surface or a tube on them is cut at.
    pub fn own_box(&self) -> [Vec3; 2] {
        let [low, high] = [0, 1].map(|end| {
            self.axes.map(|axis| {
                if end == 0 {
                    axis.range().0
                } else {
                    axis.range().1
                }
            })
        });
        let (a, b) = (self.to_own(low), self.to_own(high));
        [a.min(b), a.max(b)]
    }

    /// How far the farthest drawn point lies from the object's position, in its own space, with
    /// what lies across the frame reaching its farthest either way.
    pub fn reach(&self) -> f32 {
        let rulings = self.rulings.iter().map(|ruling| match ruling {
            Ruling::Fixed([from, to]) => from.length().max(to.length()),
            Ruling::AcrossFrame(at) => at.length() + TICK_LENGTH / 2.0,
        });
        let labels = self.labels.iter().map(|label| match label.place {
            Place::At(at) => at.length() + label.half.length(),
            Place::LeftOf(at) => {
                at.length() + TICK_LENGTH / 2.0 + LABEL_GAP + 2.0 * label.half.length()
            }
        });
        rulings.chain(labels).fold(0.0, f32::max) + self.stroke.width.get() / 2.0
    }

    /// The axis lines, then every tick, in the world as `facing` sees them.
    pub fn rulings_in_world(&self, facing: &Facing) -> Vec<(Vec3, Vec3)> {
        self.rulings
            .iter()
            .map(|ruling| match *ruling {
                Ruling::Fixed([from, to]) => (facing.world(from), facing.world(to)),
                Ruling::AcrossFrame(at) => {
                    let half = facing.right * TICK_LENGTH / 2.0 * facing.size;
                    (facing.world(at) - half, facing.world(at) + half)
                }
            })
            .collect()
    }

    /// What an agent needs to put a data point on a frame, as `Axes::report` gives for 2D axes,
    /// with each label's box as it faces `camera`.
    pub fn report(
        &self,
        facing: &Facing,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> Axes3dReport {
        let object_to_world = facing.object_to_world;
        let world = |data: [f64; 3]| facing.world(self.to_own(data));
        let spot = |data| Spot {
            world: world(data),
            pixels: to_pixels(world(data)),
        };
        let crossing = self.axes.map(|axis| axis.crossing());
        let label_box = |of: &Labelled| {
            self.labels
                .iter()
                .find(|label| label.of == *of)
                .map(|label| label.bounding_box(facing, &to_pixels))
        };
        let ticks = (0..3)
            .flat_map(|k| self.axes[k].ticks().map(move |value| (k, value)))
            .map(|(k, value)| {
                let mut data = crossing;
                data[k] = value;
                let of = Labelled::Tick {
                    axis: NAMES[k],
                    value,
                };
                let label = self
                    .labels
                    .iter()
                    .find(|label| label.of == of)
                    .map(|label| TickLabel {
                        label_text: label.text.clone(),
                        label_side: label.side.expect("a tick label has a side"),
                        label_bounding_box: label.bounding_box(facing, &to_pixels),
                    });
                Tick {
                    axis: NAMES[k],
                    value,
                    at: spot(data),
                    label,
                }
            })
            .collect();
        let axis_report = |k: usize| {
            let (low, high) = self.axes[k].range();
            let label_bounding_box = self
                .labels
                .iter()
                .find(|label| matches!(&label.of, Labelled::Axis { axis, .. } if *axis == NAMES[k]))
                .and_then(|label| label_box(&label.of));
            AxisReport {
                range: [low, high],
                spacing: self.axes[k].spacing(),
                label_bounding_box,
                categories: None,
            }
        };
        let unit = |k: usize| {
            object_to_world.transform_vector3(own_direction(k) * self.axes[k].unit() as f32)
        };
        Axes3dReport {
            x: axis_report(0),
            y: axis_report(1),
            z: axis_report(2),
            zero: spot([0.0; 3]),
            x_unit: unit(0),
            y_unit: unit(1),
            z_unit: unit(2),
            ticks,
        }
    }
}

/// The direction of the object's own space that data axis `k` runs along.
fn own_direction(k: usize) -> Vec3 {
    [Vec3::X, -Vec3::Z, Vec3::Y][k]
}

/// Where data coordinates land in the object's own space: data x along x, z up along y and y
/// along -z, each scaled and shifted. Off 3D axes a surface's own data coordinates are world
/// units, the identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataToOwn {
    /// The own units one data unit spans, along x, y and z of the data.
    pub scale: [f64; 3],
    /// Where data 0 lands along each, before y is turned to -z.
    shift: [f64; 3],
}

impl DataToOwn {
    pub const IDENTITY: DataToOwn = DataToOwn {
        scale: [1.0; 3],
        shift: [0.0; 3],
    };

    /// Each axis's range filling its length, centered on the object's position.
    fn of(axes: &[Axis; 3]) -> DataToOwn {
        let scale = axes.map(|axis| axis.unit());
        let shift = [0, 1, 2].map(|k| -(axes[k].range().0 + axes[k].range().1) / 2.0 * scale[k]);
        DataToOwn { scale, shift }
    }

    pub fn own(&self, data: [f64; 3]) -> Vec3 {
        let [x, y, z] = self.own_f64(data);
        Vec3::new(x as f32, y as f32, z as f32)
    }

    pub fn own_f64(&self, [x, y, z]: [f64; 3]) -> [f64; 3] {
        let along = |k: usize, value: f64| value * self.scale[k] + self.shift[k];
        // Adding 0 turns the -0 of a y at the middle of its range into 0.
        [along(0, x), along(2, z), 0.0 - along(1, y)]
    }

    /// The data coordinates of a point of the object's own space.
    pub fn data(&self, own: Vec3) -> [f64; 3] {
        let along = |k: usize, value: f32| (f64::from(value) - self.shift[k]) / self.scale[k];
        [along(0, own.x), along(1, -own.z), along(2, own.y)]
    }

    /// Where data z lands along the object's own y.
    pub fn own_height(&self, z: f64) -> f32 {
        (z * self.scale[2] + self.shift[2]) as f32
    }
}

/// `tex` typeset in math with its glyphs' box centered on the origin, and half that box's width
/// and height.
fn centered(tex: &str, appearance: Appearance) -> Result<(Vec<Glyph>, Vec2), TypesetError> {
    let glyphs = typeset::math(
        tex,
        appearance.color,
        Layout {
            size: appearance.label_size,
            align: Align::Center,
        },
    )?;
    let Some(bounding_box) = glyphs
        .iter()
        .map(|glyph| glyph.path.bounding_box())
        .reduce(|a, b| a.union(b))
    else {
        return Err(TypesetError::NoGlyphs);
    };
    let center = bounding_box.center();
    let glyphs = glyphs
        .into_iter()
        .map(|glyph| {
            let mut moved = BezPath::from_vec(glyph.path.elements().to_vec());
            moved.apply_affine(Affine::translate((-center.x, -center.y)));
            Glyph {
                path: Path::from_bez(moved).expect("a moved glyph has the length it had"),
                ..glyph
            }
        })
        .collect();
    Ok((
        glyphs,
        Vec2::new(bounding_box.width() as f32, bounding_box.height() as f32) / 2.0,
    ))
}

impl Label3d {
    /// Where the label's own plane lands in the world: centered where its place says, facing the
    /// camera with the frame's right and up as its x and y, and sized by the object's scale.
    pub fn to_world(&self, facing: &Facing) -> Mat4 {
        let center = match self.place {
            Place::At(at) => facing.world(at),
            Place::LeftOf(at) => {
                facing.world(at)
                    - facing.right * (TICK_LENGTH / 2.0 + LABEL_GAP + self.half.x) * facing.size
            }
        };
        let Facing {
            right,
            up,
            toward_camera,
            size,
            ..
        } = *facing;
        Mat4::from_cols(
            (right * size).extend(0.0),
            (up * size).extend(0.0),
            (toward_camera * size).extend(0.0),
            center.extend(1.0),
        )
    }

    fn bounding_box(
        &self,
        facing: &Facing,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> BoundingBox {
        let to_world = self.to_world(facing);
        let Vec2 { x, y } = self.half;
        let world: Vec<Vec3> = [(-x, -y), (x, -y), (-x, y), (x, y)]
            .into_iter()
            .map(|(a, b)| to_world.transform_point3(Vec3::new(a, b, 0.0)))
            .collect();
        let pixels = corners(
            world.iter().filter_map(|&corner| to_pixels(corner)),
            Vec2::min,
            Vec2::max,
        );
        BoundingBox {
            world: corners(world.into_iter(), Vec3::min, Vec3::max)
                .expect("a box has four corners"),
            pixels,
        }
    }
}

/// What `inspect` says of 3D axes: each axis's range, spacing and label box, where data
/// `(0, 0, 0)` lands and the world vector one data unit along each axis adds, and every tick.
#[derive(Debug, Serialize)]
pub struct Axes3dReport {
    pub x: AxisReport,
    pub y: AxisReport,
    pub z: AxisReport,
    /// The data point (0, 0, 0), even where it lies outside the ranges.
    pub zero: Spot,
    /// So data (x, y, z) is at `zero.world + x * x_unit + y * y_unit + z * z_unit`.
    pub x_unit: Vec3,
    pub y_unit: Vec3,
    pub z_unit: Vec3,
    pub ticks: Vec<Tick>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Color;

    fn axes(ranges: [(f64, f64); 3], lengths: [f32; 3]) -> Axes3d {
        let axes =
            [0, 1, 2].map(|k| Axis::new(ranges[k].0, ranges[k].1, Some(1.0), lengths[k]).unwrap());
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: Color {
                r: 255,
                g: 255,
                b: 255,
            },
            label_size: 0.3,
        };
        Axes3d::new(axes, [Some("x"), Some("y"), Some("z")], appearance).unwrap()
    }

    fn label<'a>(axes: &'a Axes3d, of: &Labelled) -> Option<&'a Label3d> {
        axes.labels.iter().find(|l| l.of == *of)
    }

    #[test]
    fn data_lands_with_x_right_z_up_and_y_away() {
        let axes = axes([(-3.0, 3.0), (0.0, 4.0), (-1.0, 1.0)], [6.0, 4.0, 2.0]);

        assert_eq!(
            axes.to_own([3.0, 0.0, 0.0]),
            Vec3::new(3.0, 0.0, 2.0),
            "x to the right; y = 0 is the near end of y"
        );
        assert_eq!(
            axes.to_own([0.0, 4.0, 0.0]),
            Vec3::new(0.0, 0.0, -2.0),
            "y away from the camera"
        );
        assert_eq!(
            axes.to_own([0.0, 2.0, 1.0]),
            Vec3::new(0.0, 1.0, 0.0),
            "z up"
        );
    }

    #[test]
    fn each_axis_crosses_the_others_at_0_or_the_nearest_end() {
        let axes = axes([(-3.0, 3.0), (1.0, 5.0), (-2.0, 2.0)], [6.0, 4.0, 4.0]);
        let facing = Facing::new(Mat4::IDENTITY, &Camera::default());

        let rulings = axes.rulings_in_world(&facing);
        assert_eq!(
            rulings[0],
            (Vec3::new(-3.0, 0.0, 2.0), Vec3::new(3.0, 0.0, 2.0)),
            "the x axis at y = 1, the low end of y, and z = 0"
        );
        assert_eq!(
            rulings[2],
            (Vec3::new(0.0, -2.0, 2.0), Vec3::new(0.0, 2.0, 2.0))
        );
    }

    #[test]
    fn a_z_tick_and_its_label_lie_across_the_frame_from_every_side() {
        let axes = axes([(0.0, 2.0), (0.0, 2.0), (0.0, 2.0)], [2.0, 2.0, 2.0]);
        let from_x = Facing::new(
            Mat4::IDENTITY,
            &Camera {
                position: Vec3::new(10.0, 0.0, 0.0),
                ..Camera::default()
            },
        );

        let z_tick = *axes.rulings_in_world(&from_x).last().unwrap();
        assert_eq!(
            z_tick,
            (
                Vec3::new(-1.0, 1.0, 1.0 + TICK_LENGTH / 2.0),
                Vec3::new(-1.0, 1.0, 1.0 - TICK_LENGTH / 2.0)
            ),
            "from +x the frame's right is -z"
        );
        let z1 = label(
            &axes,
            &Labelled::Tick {
                axis: "z",
                value: 1.0,
            },
        )
        .unwrap();
        assert_eq!(z1.side, Some(Side::Left));
        let center = z1.to_world(&from_x).transform_point3(Vec3::ZERO);
        assert!(
            center.z > 1.0 + TICK_LENGTH / 2.0 + LABEL_GAP && center.x == -1.0,
            "left of the z axis on the frame: {center}"
        );
    }

    #[test]
    fn x_and_y_tick_labels_sit_below_and_none_where_the_axes_cross() {
        let axes = axes([(0.0, 2.0), (0.0, 2.0), (0.0, 2.0)], [2.0, 2.0, 2.0]);
        let facing = Facing::new(Mat4::IDENTITY, &Camera::default());

        for axis in ["x", "y", "z"] {
            assert!(
                label(&axes, &Labelled::Tick { axis, value: 0.0 }).is_none(),
                "the crossing is unlabeled on {axis}"
            );
        }
        let x1 = label(
            &axes,
            &Labelled::Tick {
                axis: "x",
                value: 1.0,
            },
        )
        .unwrap();
        assert_eq!(x1.side, Some(Side::Below));
        let center = x1.to_world(&facing).transform_point3(Vec3::ZERO);
        assert!(
            center.y < -1.0 - TICK_LENGTH / 2.0 && center.x == 0.0,
            "below the x axis at the floor of the box: {center}"
        );
        let z_label = label(
            &axes,
            &Labelled::Axis {
                axis: "z",
                label: "z".to_string(),
            },
        )
        .unwrap();
        assert!(
            z_label.to_world(&facing).transform_point3(Vec3::ZERO).y > 1.0 + LABEL_GAP
                && z_label.side.is_none(),
            "above the top of z"
        );
        assert_eq!(
            z_label
                .glyphs
                .iter()
                .map(|glyph| glyph.color)
                .collect::<Vec<_>>(),
            [Color {
                r: 255,
                g: 255,
                b: 255
            }]
        );
    }

    #[test]
    fn a_label_faces_the_camera_at_the_object_scale() {
        let axes = axes([(0.0, 2.0), (0.0, 2.0), (0.0, 2.0)], [2.0, 2.0, 2.0]);
        let facing = Facing::new(
            Mat4::from_scale(Vec3::splat(2.0)),
            &Camera {
                position: Vec3::new(10.0, 0.0, 0.0),
                ..Camera::default()
            },
        );

        let to_world = axes.labels[0].to_world(&facing);

        assert!(
            (to_world.transform_vector3(Vec3::X) - Vec3::new(0.0, 0.0, -2.0)).length() < 1e-6,
            "the frame's right, from +x, is -z, twice as long"
        );
        assert!(
            (to_world.transform_vector3(Vec3::Z) - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-6,
            "toward the camera"
        );
    }
}
