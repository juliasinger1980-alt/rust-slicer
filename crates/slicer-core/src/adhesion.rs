use clipper2::{EndType, JoinType, One, Path as ClipperPath, inflate};
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_SCALE, Layer, Point2, Polygon, SliceError};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct AdhesionConfig {
    pub skirt_loops: usize,
    pub skirt_distance: f64,
    pub brim_enabled: bool,
    pub brim_width: f64,
    pub brim_inner_holes: bool,
}

impl Default for AdhesionConfig {
    fn default() -> Self {
        Self {
            skirt_loops: 0,
            skirt_distance: 3.0,
            brim_enabled: false,
            brim_width: 0.0,
            brim_inner_holes: false,
        }
    }
}

impl AdhesionConfig {
    pub(crate) fn validate(self) -> Result<Self, SliceError> {
        if !self.skirt_distance.is_finite()
            || !self.brim_width.is_finite()
            || self.skirt_distance < 0.0
            || self.brim_width < 0.0
        {
            return Err(SliceError::InvalidAdhesionConfiguration);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdhesionGeometry {
    pub skirts: Vec<Polygon>,
    pub brims: Vec<Polygon>,
}

pub fn generate_adhesion(
    first_layer: &Layer,
    config: AdhesionConfig,
    line_spacing: f64,
) -> Result<AdhesionGeometry, SliceError> {
    let config = config.validate()?;
    if !line_spacing.is_finite() || line_spacing <= 0.0 {
        return Err(SliceError::InvalidAdhesionConfiguration);
    }
    let outlines = first_layer
        .contours
        .iter()
        .map(simplify_polygon)
        .filter(Polygon::is_closed)
        .collect::<Vec<_>>();
    let holes = outlines
        .iter()
        .enumerate()
        .map(|(index, polygon)| {
            outlines
                .iter()
                .enumerate()
                .filter(|(candidate_index, candidate)| {
                    *candidate_index != index && point_in_polygon(polygon.points[0], candidate)
                })
                .count()
                % 2
                == 1
        })
        .collect::<Vec<_>>();
    let outer = outlines
        .iter()
        .zip(&holes)
        .filter(|(_, is_hole)| !**is_hole)
        .map(|(polygon, _)| polygon)
        .collect::<Vec<_>>();
    let inner = outlines
        .iter()
        .zip(&holes)
        .filter(|(_, is_hole)| **is_hole)
        .map(|(polygon, _)| polygon)
        .collect::<Vec<_>>();

    let mut geometry = AdhesionGeometry::default();
    for loop_index in 0..config.skirt_loops {
        let distance = config.skirt_distance + (loop_index as f64 + 0.5) * line_spacing;
        for polygon in &outer {
            geometry.skirts.extend(offset_polygon(polygon, distance)?);
        }
    }
    if config.brim_enabled && config.brim_width > 0.0 {
        let loop_count = ((config.brim_width / line_spacing) + 1e-9).floor() as usize;
        for loop_index in 0..loop_count {
            let distance = (loop_index as f64 + 0.5) * line_spacing;
            for polygon in &outer {
                geometry.brims.extend(offset_polygon(polygon, distance)?);
            }
            if config.brim_inner_holes {
                for polygon in &inner {
                    geometry.brims.extend(offset_polygon(polygon, -distance)?);
                }
            }
        }
    }
    Ok(geometry)
}

fn offset_polygon(polygon: &Polygon, distance_mm: f64) -> Result<Vec<Polygon>, SliceError> {
    let path: ClipperPath<One> = polygon
        .points
        .iter()
        .map(|point| (point.x as f64, point.y as f64))
        .collect::<Vec<_>>()
        .into();
    let paths: Vec<Vec<(f64, f64)>> = inflate(
        path,
        distance_mm * DEFAULT_SCALE,
        JoinType::Round,
        EndType::Polygon,
        2.0,
    )
    .into();
    Ok(paths
        .into_iter()
        .map(|points| Polygon {
            points: points
                .into_iter()
                .map(|(x, y)| Point2 {
                    x: x.round() as i64,
                    y: y.round() as i64,
                })
                .collect(),
        })
        .map(|polygon| simplify_polygon(&polygon))
        .filter(Polygon::is_closed)
        .collect())
}

fn simplify_polygon(polygon: &Polygon) -> Polygon {
    let mut points = polygon.points.clone();
    points.dedup();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    Polygon { points }
}

fn point_in_polygon(point: Point2, polygon: &Polygon) -> bool {
    let mut inside = false;
    for (a, b) in polygon
        .points
        .iter()
        .zip(polygon.points.iter().cycle().skip(1))
        .take(polygon.points.len())
    {
        let intersects = (a.y > point.y) != (b.y > point.y)
            && (point.x as f64)
                < (b.x as f64 - a.x as f64) * (point.y as f64 - a.y as f64)
                    / (b.y as f64 - a.y as f64)
                    + a.x as f64;
        if intersects {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(min: i64, max: i64) -> Polygon {
        Polygon {
            points: vec![
                Point2 { x: min, y: min },
                Point2 { x: max, y: min },
                Point2 { x: max, y: max },
                Point2 { x: min, y: max },
            ],
        }
    }

    fn bounds(polygon: &Polygon) -> (i64, i64, i64, i64) {
        polygon.points.iter().fold(
            (i64::MAX, i64::MIN, i64::MAX, i64::MIN),
            |(min_x, max_x, min_y, max_y), point| {
                (
                    min_x.min(point.x),
                    max_x.max(point.x),
                    min_y.min(point.y),
                    max_y.max(point.y),
                )
            },
        )
    }

    #[test]
    fn cube_skirt_has_configured_distance() {
        let geometry = generate_adhesion(
            &Layer {
                z: 0.2,
                contours: vec![square(0, 20_000)],
            },
            AdhesionConfig {
                skirt_loops: 1,
                skirt_distance: 3.0,
                ..Default::default()
            },
            0.4,
        )
        .unwrap();
        assert_eq!(
            bounds(&geometry.skirts[0]),
            (-3_200, 23_200, -3_200, 23_200)
        );
    }

    #[test]
    fn brim_loop_count_matches_printable_width() {
        let geometry = generate_adhesion(
            &Layer {
                z: 0.2,
                contours: vec![square(0, 20_000)],
            },
            AdhesionConfig {
                brim_enabled: true,
                brim_width: 1.2,
                ..Default::default()
            },
            0.4,
        )
        .unwrap();
        assert_eq!(geometry.brims.len(), 3);
    }

    #[test]
    fn outer_brim_does_not_enter_ring_hole() {
        let geometry = generate_adhesion(
            &Layer {
                z: 0.2,
                contours: vec![square(0, 20_000), square(5_000, 15_000)],
            },
            AdhesionConfig {
                brim_enabled: true,
                brim_width: 1.2,
                ..Default::default()
            },
            0.4,
        )
        .unwrap();
        assert!(geometry.brims.iter().all(|polygon| {
            polygon.points.iter().all(|point| {
                point.x < 5_000 || point.x > 15_000 || point.y < 5_000 || point.y > 15_000
            })
        }));
    }
}
