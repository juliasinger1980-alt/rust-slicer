use std::f64::consts::PI;

use clipper2::{EndType, JoinType, One, Path as ClipperPath, inflate};
use rayon::prelude::*;

use crate::{DEFAULT_SCALE, PerimeterLayer, Point2, Polygon, SliceError, WallLoop};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfillConfig {
    pub density: f64,
    pub line_width: f64,
    pub base_angle: f64,
    pub wall_overlap: f64,
}

impl Default for InfillConfig {
    fn default() -> Self {
        Self {
            density: 0.2,
            line_width: 0.4,
            base_angle: 0.0,
            wall_overlap: 0.05,
        }
    }
}

impl InfillConfig {
    fn validate(self) -> Result<Self, SliceError> {
        if !self.density.is_finite()
            || !self.line_width.is_finite()
            || !self.base_angle.is_finite()
            || !self.wall_overlap.is_finite()
            || !(0.0 < self.density && self.density <= 1.0)
            || self.line_width <= 0.0
            || self.wall_overlap < 0.0
            || self.wall_overlap >= self.line_width * 0.5
        {
            return Err(SliceError::InvalidInfillConfiguration);
        }
        Ok(self)
    }

    pub fn line_spacing(self) -> Result<f64, SliceError> {
        self.validate()
            .map(|config| config.line_width / config.density)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InfillLine {
    pub start: Point2,
    pub end: Point2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InfillLayer {
    pub z: f64,
    pub angle: f64,
    pub lines: Vec<InfillLine>,
}

pub fn generate_infill(
    layers: &[PerimeterLayer],
    config: InfillConfig,
) -> Result<Vec<InfillLayer>, SliceError> {
    let config = config.validate()?;
    Ok(layers
        .par_iter()
        .enumerate()
        .map(|(layer_index, layer)| {
            let angle =
                config.base_angle.to_radians() + if layer_index % 2 == 0 { 0.0 } else { PI / 2.0 };
            InfillLayer {
                z: layer.z,
                angle,
                lines: generate_layer_infill(layer, config, angle),
            }
        })
        .collect())
}

fn generate_layer_infill(
    layer: &PerimeterLayer,
    config: InfillConfig,
    angle: f64,
) -> Vec<InfillLine> {
    let contours = infill_contours(&layer.walls, config);
    if contours.is_empty() {
        return Vec::new();
    }

    let rotated = contours
        .iter()
        .map(|(polygon, is_hole)| (rotate_polygon(polygon, -angle), *is_hole))
        .collect::<Vec<_>>();
    let (min_x, max_x) = rotated
        .iter()
        .flat_map(|(polygon, _)| polygon.iter().map(|point| point.0))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
            (min.min(x), max.max(x))
        });
    let spacing = config.line_width * DEFAULT_SCALE / config.density;
    let first = (min_x / spacing).ceil() * spacing;
    let mut lines = Vec::new();
    let mut x = first;
    while x <= max_x + 1e-6 {
        let mut crossings = Vec::new();
        for (polygon, _) in &rotated {
            for (a, b) in polygon
                .iter()
                .zip(polygon.iter().cycle().skip(1))
                .take(polygon.len())
            {
                if (a.0 <= x && x < b.0) || (b.0 <= x && x < a.0) {
                    let t = (x - a.0) / (b.0 - a.0);
                    crossings.push(a.1 + t * (b.1 - a.1));
                }
            }
        }
        crossings.sort_by(f64::total_cmp);
        for &[y0, y1] in crossings.as_chunks::<2>().0 {
            if y1 - y0 <= 1.0 {
                continue;
            }
            let midpoint = (x, (y0 + y1) * 0.5);
            if inside_fill(midpoint, &rotated) {
                let start = quantize(rotate_point((x, y0), angle));
                let end = quantize(rotate_point((x, y1), angle));
                if start != end {
                    lines.push(InfillLine { start, end });
                }
            }
        }
        x += spacing;
    }
    lines
}

fn infill_contours(walls: &[WallLoop], config: InfillConfig) -> Vec<(Vec<(f64, f64)>, bool)> {
    let max_wall = walls.iter().map(|wall| wall.wall_index).max();
    let Some(max_wall) = max_wall else {
        return Vec::new();
    };
    let boundary_offset = (config.line_width * 0.5 - config.wall_overlap) * DEFAULT_SCALE;
    walls
        .iter()
        .filter(|wall| wall.wall_index == max_wall)
        .filter_map(|wall| {
            let distance = if wall.is_hole {
                boundary_offset
            } else {
                -boundary_offset
            };
            let polygon = offset_polygon(&wall.polygon, distance)?;
            Some((polygon, wall.is_hole))
        })
        .collect()
}

fn offset_polygon(polygon: &Polygon, distance: f64) -> Option<Vec<(f64, f64)>> {
    let path: ClipperPath<One> = polygon
        .points
        .iter()
        .map(|point| (point.x as f64, point.y as f64))
        .collect::<Vec<_>>()
        .into();
    let offset: Vec<Vec<(f64, f64)>> =
        inflate(path, distance, JoinType::Miter, EndType::Polygon, 2.0).into();
    offset.into_iter().find(|points| points.len() >= 3)
}

fn rotate_polygon(polygon: &[(f64, f64)], angle: f64) -> Vec<(f64, f64)> {
    polygon
        .iter()
        .map(|&point| rotate_point(point, angle))
        .collect()
}

fn rotate_point(point: (f64, f64), angle: f64) -> (f64, f64) {
    let (sin, cos) = angle.sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

fn inside_fill(point: (f64, f64), contours: &[(Vec<(f64, f64)>, bool)]) -> bool {
    let inside_outer = contours
        .iter()
        .any(|(polygon, is_hole)| !is_hole && point_in_polygon(point, polygon));
    let inside_hole = contours
        .iter()
        .any(|(polygon, is_hole)| *is_hole && point_in_polygon(point, polygon));
    inside_outer && !inside_hole
}

fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    for (a, b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        if (a.1 > point.1) != (b.1 > point.1)
            && point.0 < (b.0 - a.0) * (point.1 - a.1) / (b.1 - a.1) + a.0
        {
            inside = !inside;
        }
    }
    inside
}

fn quantize(point: (f64, f64)) -> Point2 {
    Point2 {
        x: point.0.round() as i64,
        y: point.1.round() as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, PerimeterConfig};

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

    #[test]
    fn twenty_percent_cube_uses_two_millimeter_spacing() {
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![square(0, 20_000)],
        }];
        let perimeters = crate::generate_perimeters(&layers, PerimeterConfig::default()).unwrap();
        let config = InfillConfig::default();
        let infill = generate_infill(&perimeters, config).unwrap();

        assert_eq!(config.line_spacing().unwrap(), 2.0);
        assert_eq!(infill[0].lines.len(), 9);
    }

    #[test]
    fn infill_angle_alternates_by_ninety_degrees() {
        let layers = vec![
            PerimeterLayer {
                z: 0.2,
                walls: Vec::new(),
            },
            PerimeterLayer {
                z: 0.4,
                walls: Vec::new(),
            },
        ];

        let infill = generate_infill(&layers, InfillConfig::default()).unwrap();

        assert_eq!(infill[0].angle, 0.0);
        assert!((infill[1].angle - PI / 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ring_infill_is_split_around_the_hole() {
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![square(0, 20_000), square(5_000, 15_000)],
        }];
        let perimeters = crate::generate_perimeters(
            &layers,
            PerimeterConfig {
                wall_count: 2,
                ..PerimeterConfig::default()
            },
        )
        .unwrap();
        let infill = generate_infill(&perimeters, InfillConfig::default()).unwrap();

        assert!(infill[0].lines.iter().all(|line| {
            let midpoint = (
                (line.start.x + line.end.x) as f64 * 0.5,
                (line.start.y + line.end.y) as f64 * 0.5,
            );
            !(4_250.0 < midpoint.0
                && midpoint.0 < 15_750.0
                && 4_250.0 < midpoint.1
                && midpoint.1 < 15_750.0)
        }));
        assert!(infill[0].lines.len() > 9);
    }

    #[test]
    fn very_thin_part_produces_no_infill_without_panicking() {
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![square(0, 500)],
        }];
        let perimeters = crate::generate_perimeters(&layers, PerimeterConfig::default()).unwrap();

        let infill = generate_infill(&perimeters, InfillConfig::default()).unwrap();

        assert!(infill[0].lines.is_empty());
    }
}
