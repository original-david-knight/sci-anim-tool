//! A trail: the world points an object passed through over the last `length` seconds of scene
//! time, or since it began to show. The points are the object's world position at every frame
//! time, worked out once when the scene file is read, so a trail at any scene time is a lookup
//! and the frame stays a pure function of the scene and the scene time.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use serde::Serialize;

use crate::scene::{ObjectId, Stroke, frame_time};

#[derive(Clone, Debug)]
pub struct Trail {
    /// The object whose path the trail draws. Not a trail, and not a plot.
    pub object: ObjectId,
    /// How many seconds back the trail reaches, or `None` for every second since `since`.
    pub length: Option<f64>,
    pub stroke: Stroke,
    /// The object's world position at each frame time of the scene, frame 0 first, once the
    /// timeline is read. Shared, because a stage is cloned for every frame.
    pub passed: Arc<[Vec3]>,
    pub fps: f64,
    /// The scene time the object begins to show: 0, or the start of its create, or the end of
    /// the reaction or morph that makes it.
    pub since: f64,
    /// The points drawn at the stage's scene time, oldest first, the newest where the object is
    /// then.
    pub points: Vec<Vec3>,
}

impl Trail {
    /// The points the trail draws at `scene_time`, with its object at `now`: its position at each
    /// frame time from `length` seconds back, or from `since`, up to but not at `scene_time`,
    /// then `now`. Points where the object held still are one point.
    pub fn points_at(&self, scene_time: f64, now: Vec3) -> Vec<Vec3> {
        let from = self
            .length
            .map_or(self.since, |length| (scene_time - length).max(self.since));
        if scene_time < self.since {
            return Vec::new();
        }
        let first = (from * self.fps).ceil().max(0.0) as u32;
        let mut points: Vec<Vec3> = (first..)
            .map(|i| (i, frame_time(i, self.fps)))
            .take_while(|(i, time)| *time < scene_time && (*i as usize) < self.passed.len())
            .map(|(i, _)| self.passed[i as usize])
            .chain(std::iter::once(now))
            .collect();
        points.dedup();
        points
    }
}

/// What `inspect` says of a trail: how many points it draws and where its newest is, the point
/// its object stands at, with the pixel it lands on.
#[derive(Debug, Serialize)]
pub struct TrailReport {
    pub point_count: usize,
    pub newest: Option<Vec3>,
    pub newest_pixels: Option<Vec2>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Stroke;

    fn trail(length: Option<f64>, since: f64) -> Trail {
        let passed: Vec<Vec3> = (0..10).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        Trail {
            object: ObjectId::new("p"),
            length,
            stroke: Stroke::default(),
            passed: passed.into(),
            fps: 1.0,
            since,
            points: Vec::new(),
        }
    }

    #[test]
    fn a_trail_reaches_back_its_length_and_ends_where_its_object_is() {
        let points = trail(Some(3.0), 0.0).points_at(5.5, Vec3::new(5.5, 0.0, 0.0));

        assert_eq!(points, [3.0, 4.0, 5.0, 5.5].map(|x| Vec3::new(x, 0.0, 0.0)));
    }

    #[test]
    fn a_trail_with_no_length_reaches_back_to_when_its_object_began_to_show() {
        assert_eq!(
            trail(None, 2.0)
                .points_at(4.0, Vec3::new(4.0, 0.0, 0.0))
                .len(),
            3,
            "frames 2 and 3, then now"
        );
        assert!(
            trail(None, 2.0).points_at(1.0, Vec3::ZERO).is_empty(),
            "nothing before its object shows"
        );
    }
}
