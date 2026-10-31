//! Where a data point lands: the data coordinates of axes or 3D axes in the world at one scene
//! time. A dot on axes and a sphere on 3D axes keep their position in data coordinates, which the
//! timeline animates, and the stage carries it to the world through `DataToWorld`.

use glam::{Mat4, Vec3};

use crate::axes::Axes;
use crate::axes_3d::DataToOwn;
use crate::scene::{Object, Shape};

/// Where the data coordinates of axes or 3D axes land in the world at one scene time: what
/// places a data point, and what turns a carry's world point back into data.
#[derive(Clone, Copy, Debug)]
pub struct DataToWorld<'a> {
    to_own: ToOwn<'a>,
    own_to_world: Mat4,
}

#[derive(Clone, Copy, Debug)]
enum ToOwn<'a> {
    /// x and y land where a plot's do, and z is the distance off the axes' plane.
    Axes(&'a Axes),
    Axes3d(DataToOwn),
}

impl<'a> DataToWorld<'a> {
    /// `axes` is an axes or 3D axes object as a stage holds it.
    pub fn of(axes: &'a Object) -> DataToWorld<'a> {
        match &axes.shape {
            Shape::Axes(flat) => {
                // A plot is placed by its axes' plane map too, so a data point goes where it goes.
                // Its z stays in world units however the axes are scaled, as its radius does.
                let mut own_to_world = axes.flat_to_world().to_world();
                own_to_world.z_axis = own_to_world
                    .z_axis
                    .truncate()
                    .normalize_or_zero()
                    .extend(0.0);
                DataToWorld {
                    to_own: ToOwn::Axes(flat),
                    own_to_world,
                }
            }
            Shape::Axes3d(axes_3d) => DataToWorld {
                to_own: ToOwn::Axes3d(axes_3d.data_to_own()),
                own_to_world: axes.transform.object_to_world(),
            },
            _ => unreachable!("`scene_file` checked that a data point names axes or 3D axes"),
        }
    }

    pub fn world(&self, data: Vec3) -> Vec3 {
        let own = match self.to_own {
            ToOwn::Axes(axes) => axes
                .to_local((f64::from(data.x), f64::from(data.y)))
                .extend(data.z),
            ToOwn::Axes3d(to_own) => to_own.own(data.to_array().map(f64::from)),
        };
        self.own_to_world.transform_point3(own)
    }

    /// The data coordinates of a world point, or `None` while the axes are scaled flat and have
    /// none.
    pub fn data(&self, world: Vec3) -> Option<Vec3> {
        if self.own_to_world.determinant() == 0.0 {
            return None;
        }
        let own = self.own_to_world.inverse().transform_point3(world);
        Some(match self.to_own {
            ToOwn::Axes(axes) => {
                let (x, y) = axes.to_data(own.truncate());
                Vec3::new(x as f32, y as f32, own.z)
            }
            ToOwn::Axes3d(to_own) => Vec3::from_array(to_own.data(own).map(|v| v as f32)),
        })
    }
}
