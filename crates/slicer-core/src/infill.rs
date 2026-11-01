use std::f64::consts::PI;

use clipper2::{
    EndType, FillRule, JoinType, One, Path as ClipperPath, difference, inflate, intersect, union,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_SCALE, PerimeterLayer, Point2, Polygon, SliceError, WallLoop};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
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
            base_angle: 45.0,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SolidInfillConfig {
    pub top_layers: usize,
    pub bottom_layers: usize,
    pub solid_line_width: f64,
}

impl Default for SolidInfillConfig {
    fn default() -> Self {
        Self {
            top_layers: 4,
            bottom_layers: 4,
            solid_line_width: 0.4,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolidInfillLayer {
    pub z: f64,
    pub sparse_region: Vec<Polygon>,
    pub solid_region: Vec<Polygon>,
    pub sparse_lines: Vec<InfillLine>,
    pub solid_lines: Vec<InfillLine>,
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
                lines: generate_lines(
                    infill_contours(&layer.walls, config),
                    config.line_width,
                    config.density,
                    angle,
                ),
            }
        })
        .collect())
}

pub fn generate_solid_infills(
    layers: &[PerimeterLayer],
    infill_config: InfillConfig,
    solid_config: SolidInfillConfig,
) -> Result<Vec<SolidInfillLayer>, SliceError> {
    infill_config.validate()?;
    if !solid_config.solid_line_width.is_finite() || solid_config.solid_line_width <= 0.0 {
        return Err(SliceError::InvalidInfillConfiguration);
    }
    let printable = layers
        .iter()
        .map(|layer| infill_contours(&layer.walls, infill_config))
        .collect::<Vec<_>>();
    let paths = printable
        .iter()
        .map(|contours| contours_to_paths(contours))
        .collect::<Vec<_>>();
    let comparison_tolerance = (infill_config.line_width * 0.25 * DEFAULT_SCALE).max(1.0);
    let top = paths
        .iter()
        .enumerate()
        .map(|(index, current)| {
            paths.get(index + 1).map_or_else(
                || Ok(current.clone()),
                |above| exposed_area(current, above, comparison_tolerance),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bottom = paths
        .iter()
        .enumerate()
        .map(|(index, current)| {
            index.checked_sub(1).map_or_else(
                || Ok(current.clone()),
                |below| exposed_area(current, &paths[below], comparison_tolerance),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let layers = layers
        .par_iter()
        .enumerate()
        .map(|(index, layer)| {
            let mut solid = Vec::new();
            for offset in 0..solid_config.top_layers {
                if let Some(seed) = top.get(index + offset) {
                    solid = union_paths(&solid, seed)?;
                }
            }
            for offset in 0..solid_config.bottom_layers {
                if let Some(seed_index) = index.checked_sub(offset) {
                    solid = union_paths(&solid, &bottom[seed_index])?;
                }
            }
            solid = intersect_paths(&solid, &paths[index])?;
            solid = opening_paths(
                &solid,
                (solid_config.solid_line_width * DEFAULT_SCALE).max(1.0),
                (solid_config.solid_line_width
                    * DEFAULT_SCALE
                    * solid_config.solid_line_width
                    * DEFAULT_SCALE
                    * 0.25)
                    .max(1.0),
            )?;
            let sparse = difference_paths(&paths[index], &solid)?;
            let angle =
                infill_config.base_angle.to_radians() + if index % 2 == 0 { 0.0 } else { PI / 2.0 };
            Ok(SolidInfillLayer {
                z: layer.z,
                sparse_region: raw_paths_to_polygons(&sparse),
                solid_region: raw_paths_to_polygons(&solid),
                sparse_lines: generate_lines(
                    raw_paths_to_contours(&sparse),
                    infill_config.line_width,
                    infill_config.density,
                    angle,
                ),
                solid_lines: generate_lines(
                    raw_paths_to_contours(&solid),
                    solid_config.solid_line_width,
                    1.0,
                    angle,
                ),
            })
        })
        .collect::<Result<Vec<_>, SliceError>>()?;
    Ok(layers)
}

type RawPaths = Vec<Vec<(f64, f64)>>;

fn generate_lines(
    contours: Vec<(Vec<(f64, f64)>, bool)>,
    line_width: f64,
    density: f64,
    angle: f64,
) -> Vec<InfillLine> {
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
    let spacing = line_width * DEFAULT_SCALE / density;
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

fn contours_to_paths(contours: &[(Vec<(f64, f64)>, bool)]) -> RawPaths {
    contours
        .iter()
        .map(|(polygon, _)| polygon.clone())
        .collect()
}

fn contours_to_polygons(contours: &[(Vec<(f64, f64)>, bool)]) -> Vec<Polygon> {
    contours
        .iter()
        .map(|(points, _)| Polygon {
            points: points
                .iter()
                .map(|&(x, y)| Point2 {
                    x: x.round() as i64,
                    y: y.round() as i64,
                })
                .collect(),
        })
        .filter(Polygon::is_closed)
        .collect()
}

fn raw_paths_to_polygons(paths: &RawPaths) -> Vec<Polygon> {
    let contours = raw_paths_to_contours(paths);
    contours_to_polygons(&contours)
}

fn raw_paths_to_contours(paths: &RawPaths) -> Vec<(Vec<(f64, f64)>, bool)> {
    paths
        .iter()
        .filter(|path| path.len() >= 3)
        .map(|path| {
            let point = path[0];
            let containing = paths
                .iter()
                .filter(|candidate| candidate.len() >= 3 && !std::ptr::eq(*candidate, path))
                .filter(|candidate| point_in_polygon(point, candidate))
                .count();
            (path.clone(), containing % 2 == 1)
        })
        .collect()
}

fn difference_paths(subject: &RawPaths, clip: &RawPaths) -> Result<RawPaths, SliceError> {
    if subject.is_empty() {
        return Ok(Vec::new());
    }
    if clip.is_empty() {
        return Ok(subject.clone());
    }
    difference::<One>(subject.clone(), clip.clone(), FillRule::EvenOdd)
        .map(|paths: clipper2::Paths<One>| paths.into())
        .map_err(|error| SliceError::BooleanOperation(error.to_string()))
}

fn exposed_area(
    current: &RawPaths,
    neighbor: &RawPaths,
    tolerance: f64,
) -> Result<RawPaths, SliceError> {
    let comparison_area = offset_paths(current, -tolerance)?;
    difference_paths(&comparison_area, neighbor)
}

fn opening_paths(
    paths: &RawPaths,
    opening_width: f64,
    minimum_area: f64,
) -> Result<RawPaths, SliceError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let eroded = offset_paths(paths, -opening_width)?;
    if eroded.is_empty() {
        return Ok(Vec::new());
    }
    let opened = offset_paths(&eroded, opening_width)?;
    filter_small_regions(&opened, minimum_area)
}

fn offset_paths(paths: &RawPaths, delta: f64) -> Result<RawPaths, SliceError> {
    Ok(inflate::<One>(paths.clone(), delta, JoinType::Miter, EndType::Polygon, 2.0).into())
}

fn filter_small_regions(paths: &RawPaths, minimum_area: f64) -> Result<RawPaths, SliceError> {
    let mut retained = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let point = path[0];
        let containing = paths
            .iter()
            .enumerate()
            .filter(|(candidate_index, candidate)| {
                *candidate_index != index
                    && candidate.len() >= 3
                    && point_in_polygon(point, candidate)
            })
            .count();
        let is_hole = containing % 2 == 1;
        let retain = (!is_hole && polygon_area(path) >= minimum_area)
            || (is_hole
                && paths.iter().enumerate().any(|(outer_index, outer)| {
                    outer_index != index
                        && polygon_area(outer) >= minimum_area
                        && point_in_polygon(point, outer)
                }));
        if retain {
            retained.push(path.clone());
        }
    }
    if retained.is_empty() {
        return Ok(Vec::new());
    }
    union_paths(&retained, &Vec::new())
}

fn polygon_area(path: &[(f64, f64)]) -> f64 {
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        .abs()
        * 0.5
}

fn intersect_paths(subject: &RawPaths, clip: &RawPaths) -> Result<RawPaths, SliceError> {
    if subject.is_empty() || clip.is_empty() {
        return Ok(Vec::new());
    }
    intersect::<One>(subject.clone(), clip.clone(), FillRule::EvenOdd)
        .map(|paths: clipper2::Paths<One>| paths.into())
        .map_err(|error| SliceError::BooleanOperation(error.to_string()))
}

fn union_paths(subject: &RawPaths, clip: &RawPaths) -> Result<RawPaths, SliceError> {
    if subject.is_empty() {
        return Ok(clip.clone());
    }
    if clip.is_empty() {
        return Ok(subject.clone());
    }
    union::<One>(subject.clone(), clip.clone(), FillRule::EvenOdd)
        .map(|paths: clipper2::Paths<One>| paths.into())
        .map_err(|error| SliceError::BooleanOperation(error.to_string()))
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
    contours
        .iter()
        .filter(|(polygon, _)| point_in_polygon(point, polygon))
        .count()
        % 2
        == 1
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

    fn perimeter_layer(z: f64, polygons: &[(Polygon, bool)]) -> PerimeterLayer {
        PerimeterLayer {
            z,
            walls: polygons
                .iter()
                .map(|(polygon, is_hole)| WallLoop {
                    wall_index: 0,
                    is_hole: *is_hole,
                    polygon: polygon.clone(),
                })
                .collect(),
        }
    }

    fn circle(center: i64, radius: i64) -> Polygon {
        let points = (0..64)
            .map(|index| {
                let angle = index as f64 * 2.0 * PI / 64.0;
                Point2 {
                    x: center + (radius as f64 * angle.cos()).round() as i64,
                    y: center + (radius as f64 * angle.sin()).round() as i64,
                }
            })
            .collect::<Vec<_>>();
        Polygon { points }
    }

    #[test]
    fn middle_layers_have_no_solid_infill_on_straight_shapes() {
        let ring_layers = (0..100)
            .map(|index| Layer {
                z: index as f64 * 0.2,
                contours: vec![circle(20_000, 20_000), circle(20_000, 8_000)],
            })
            .collect::<Vec<_>>();
        let cylinder_layers = (0..100)
            .map(|index| Layer {
                z: index as f64 * 0.2,
                contours: vec![circle(20_000, 20_000)],
            })
            .collect::<Vec<_>>();
        for layers in [ring_layers, cylinder_layers] {
            let perimeters =
                crate::generate_perimeters(&layers, PerimeterConfig::default()).unwrap();
            let solid = generate_solid_infills(
                &perimeters,
                InfillConfig::default(),
                SolidInfillConfig::default(),
            )
            .unwrap();
            assert!(
                solid[4..96]
                    .iter()
                    .all(|layer| layer.solid_lines.is_empty())
            );
        }
    }

    #[test]
    fn four_shell_layers_are_solid_only_at_cube_boundaries() {
        let layers = (0..100)
            .map(|index| perimeter_layer(index as f64 * 0.2, &[(square(0, 20_000), false)]))
            .collect::<Vec<_>>();
        let solid = generate_solid_infills(
            &layers,
            InfillConfig::default(),
            SolidInfillConfig::default(),
        )
        .unwrap();

        assert!(
            solid[..4]
                .iter()
                .all(|layer| !layer.solid_region.is_empty())
        );
        assert!(
            solid[96..]
                .iter()
                .all(|layer| !layer.solid_region.is_empty())
        );
        assert!(
            solid[4..96]
                .iter()
                .all(|layer| layer.solid_region.is_empty())
        );
        assert!(
            solid[4..96]
                .iter()
                .all(|layer| !layer.sparse_region.is_empty())
        );
    }

    #[test]
    fn through_hole_is_preserved_in_solid_layers() {
        let layers = (0..2)
            .map(|index| Layer {
                z: 0.2 + index as f64 * 0.2,
                contours: vec![square(0, 20_000), square(5_000, 15_000)],
            })
            .collect::<Vec<_>>();
        let perimeters = crate::generate_perimeters(&layers, PerimeterConfig::default()).unwrap();
        let solid = generate_solid_infills(
            &perimeters,
            InfillConfig::default(),
            SolidInfillConfig {
                top_layers: 1,
                bottom_layers: 1,
                ..SolidInfillConfig::default()
            },
        )
        .unwrap();

        for layer in &solid {
            assert!(
                layer
                    .solid_lines
                    .iter()
                    .chain(&layer.sparse_lines)
                    .all(|line| {
                        let midpoint = (
                            (line.start.x + line.end.x) as f64 / 2.0,
                            (line.start.y + line.end.y) as f64 / 2.0,
                        );
                        !(5_000.0 < midpoint.0
                            && midpoint.0 < 15_000.0
                            && 5_000.0 < midpoint.1
                            && midpoint.1 < 15_000.0)
                    })
            );
        }
    }

    #[test]
    fn overhang_gets_solid_infill_under_its_roof() {
        let layers = vec![
            perimeter_layer(0.2, &[(square(0, 10_000), false)]),
            perimeter_layer(0.4, &[(square(0, 20_000), false)]),
            perimeter_layer(0.6, &[(square(0, 20_000), false)]),
        ];
        let solid = generate_solid_infills(
            &layers,
            InfillConfig::default(),
            SolidInfillConfig {
                top_layers: 0,
                bottom_layers: 2,
                ..SolidInfillConfig::default()
            },
        )
        .unwrap();

        assert!(!solid[1].solid_lines.is_empty());
        assert!(!solid[2].solid_lines.is_empty());
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
        assert_eq!(infill[0].lines.len(), 13);
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

        assert_eq!(infill[0].angle, PI / 4.0);
        assert!((infill[1].angle - 3.0 * PI / 4.0).abs() < f64::EPSILON);
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
