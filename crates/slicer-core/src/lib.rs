use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use clipper2::{EndType, JoinType, One, Path as ClipperPath, inflate};
use rayon::prelude::*;

pub mod debug;
pub mod infill;
mod model;

pub use debug::write_infill_layer_svg;
pub use infill::{InfillConfig, InfillLayer, InfillLine, generate_infill};
pub use model::{
    Layer, Mesh, PerimeterConfig, PerimeterLayer, Point2, Polygon, SliceConfig, SliceError,
    Triangle, Vertex3, WallLoop,
};

pub const DEFAULT_SCALE: f64 = 1_000.0;

pub fn slice_mesh(mesh: &Mesh, config: SliceConfig) -> Result<Vec<Layer>, SliceError> {
    mesh.validate()?;
    let config = config.validate()?;
    let min_z = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(f64::INFINITY, f64::min);
    let max_z = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut zs = Vec::new();
    let mut z = min_z + config.first_layer_height;
    while z <= max_z + f64::EPSILON {
        zs.push(z);
        z += config.layer_height;
    }
    Ok(zs
        .par_iter()
        .map(|&z| Layer {
            z,
            contours: slice_plane(mesh, z, config.scale),
        })
        .collect())
}

pub fn generate_perimeters(
    layers: &[Layer],
    config: PerimeterConfig,
) -> Result<Vec<PerimeterLayer>, SliceError> {
    let config = config.validate()?;
    Ok(layers
        .par_iter()
        .map(|layer| PerimeterLayer {
            z: layer.z,
            walls: generate_layer_perimeters(layer, config),
        })
        .collect())
}

fn generate_layer_perimeters(layer: &Layer, config: PerimeterConfig) -> Vec<WallLoop> {
    let outlines = layer
        .contours
        .iter()
        .map(simplify_polygon)
        .filter(|polygon| polygon.is_closed())
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

    let mut walls = Vec::new();
    for wall_index in 0..config.wall_count {
        let distance = config.line_width * (wall_index as f64 + 0.5);
        for (outline, &is_hole) in outlines.iter().zip(&holes) {
            let delta = if is_hole { distance } else { -distance };
            for polygon in offset_polygon(outline, delta) {
                walls.push(WallLoop {
                    wall_index,
                    is_hole,
                    polygon,
                });
            }
        }
    }
    walls
}

fn simplify_polygon(polygon: &Polygon) -> Polygon {
    let mut points = polygon.points.clone();
    points.dedup();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    loop {
        if points.len() < 3 {
            break;
        }
        let mut removed = false;
        for index in 0..points.len() {
            let previous = points[(index + points.len() - 1) % points.len()];
            let current = points[index];
            let next = points[(index + 1) % points.len()];
            let ab = (
                i128::from(previous.x) - i128::from(current.x),
                i128::from(previous.y) - i128::from(current.y),
            );
            let cb = (
                i128::from(next.x) - i128::from(current.x),
                i128::from(next.y) - i128::from(current.y),
            );
            if ab.0 * cb.1 - ab.1 * cb.0 == 0 && ab.0 * cb.0 + ab.1 * cb.1 <= 0 {
                points.remove(index);
                removed = true;
                break;
            }
        }
        if !removed {
            break;
        }
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

fn offset_polygon(polygon: &Polygon, distance: f64) -> Vec<Polygon> {
    let path: ClipperPath<One> = polygon
        .points
        .iter()
        .map(|point| (point.x as f64, point.y as f64))
        .collect::<Vec<_>>()
        .into();
    let offset = inflate(
        path,
        distance * DEFAULT_SCALE,
        JoinType::Miter,
        EndType::Polygon,
        2.0,
    );
    let paths: Vec<Vec<(f64, f64)>> = offset.into();
    paths
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
        .filter(|polygon| polygon.is_closed())
        .collect()
}

fn slice_plane(mesh: &Mesh, z: f64, scale: f64) -> Vec<Polygon> {
    let mut horizontal_edges = HashSet::new();
    let mut non_horizontal_edges = HashSet::new();
    for triangle in &mesh.triangles {
        let vertices = triangle.vertices.map(|index| mesh.vertices[index]);
        let on_plane = vertices.map(|vertex| nearly_equal(vertex.z, z));
        let mut points = Vec::new();
        for index in 0..3 {
            let next = (index + 1) % 3;
            if on_plane[index] {
                points.push(to_point(vertices[index], scale));
            } else if (vertices[index].z < z) != (vertices[next].z < z) && !on_plane[next] {
                points.push(intersection(vertices[index], vertices[next], z, scale));
            }
        }
        deduplicate_points(&mut points);

        if on_plane.iter().all(|&value| value) {
            let normal_z = (vertices[1].x - vertices[0].x) * (vertices[2].y - vertices[0].y)
                - (vertices[1].y - vertices[0].y) * (vertices[2].x - vertices[0].x);
            if normal_z > 0.0 {
                for index in 0..3 {
                    let edge = ordered_edge(
                        to_point(vertices[index], scale),
                        to_point(vertices[(index + 1) % 3], scale),
                    );
                    toggle_edge(&mut horizontal_edges, edge);
                }
            }
        } else if points.len() == 2
            && points[0] != points[1]
            && !(on_plane.iter().filter(|&&value| value).count() == 2
                && vertices
                    .iter()
                    .find(|vertex| !nearly_equal(vertex.z, z))
                    .is_some_and(|vertex| vertex.z > z))
        {
            non_horizontal_edges.insert(ordered_edge(points[0], points[1]));
        }
    }
    let segments = horizontal_edges
        .into_iter()
        .chain(non_horizontal_edges)
        .collect();
    build_loops(segments)
}

fn nearly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9
}

fn to_point(vertex: Vertex3, scale: f64) -> Point2 {
    Point2 {
        x: (vertex.x * scale).round() as i64,
        y: (vertex.y * scale).round() as i64,
    }
}

fn intersection(a: Vertex3, b: Vertex3, z: f64, scale: f64) -> Point2 {
    let t = (z - a.z) / (b.z - a.z);
    to_point(
        Vertex3 {
            x: a.x + t * (b.x - a.x),
            y: a.y + t * (b.y - a.y),
            z,
        },
        scale,
    )
}

fn deduplicate_points(points: &mut Vec<Point2>) {
    points.dedup();
}

fn ordered_edge(a: Point2, b: Point2) -> (Point2, Point2) {
    if a <= b { (a, b) } else { (b, a) }
}

fn toggle_edge(edges: &mut HashSet<(Point2, Point2)>, edge: (Point2, Point2)) {
    if !edges.remove(&edge) {
        edges.insert(edge);
    }
}

fn build_loops(segments: HashSet<(Point2, Point2)>) -> Vec<Polygon> {
    let mut adjacency: HashMap<Point2, Vec<Point2>> = HashMap::new();
    for (a, b) in &segments {
        adjacency.entry(*a).or_default().push(*b);
        adjacency.entry(*b).or_default().push(*a);
    }
    let mut unused = segments;
    let mut polygons = Vec::new();
    while let Some(&(start, next)) = unused.iter().next() {
        unused.remove(&(start, next));
        let mut points = vec![start, next];
        let mut previous = start;
        let mut current = next;
        while current != start {
            let Some(neighbors) = adjacency.get(&current) else {
                break;
            };
            let Some(&candidate) = neighbors.iter().find(|&&point| {
                point != previous && unused.contains(&ordered_edge(current, point))
            }) else {
                break;
            };
            unused.remove(&ordered_edge(current, candidate));
            points.push(candidate);
            previous = current;
            current = candidate;
        }
        if points.last() == Some(&start) {
            points.pop();
        }
        let polygon = Polygon { points };
        if polygon.is_closed() {
            polygons.push(polygon);
        }
    }
    polygons
}

pub fn write_layer_svg(layer: &Layer, path: impl AsRef<Path>) -> Result<(), SliceError> {
    let coordinates = layer
        .contours
        .iter()
        .flat_map(|polygon| polygon.points.iter())
        .map(|point| {
            (
                point.x as f64 / DEFAULT_SCALE,
                point.y as f64 / DEFAULT_SCALE,
            )
        })
        .collect::<Vec<_>>();
    let (min_x, max_x, min_y, max_y) = coordinates.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), &(x, y)| {
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    );
    let (min_x, max_x, min_y, max_y) = if coordinates.is_empty() {
        (0.0, 1.0, 0.0, 1.0)
    } else {
        (min_x, max_x, min_y, max_y)
    };
    let margin = 1.0;
    let view_min_x = min_x - margin;
    let view_min_y = min_y - margin;
    let view_width = (max_x - min_x + 2.0 * margin).max(1.0);
    let view_height = (max_y - min_y + 2.0 * margin).max(1.0);
    let mut data = String::new();
    for polygon in &layer.contours {
        for (index, point) in polygon.points.iter().enumerate() {
            let x = point.x as f64 / DEFAULT_SCALE;
            let y = point.y as f64 / DEFAULT_SCALE;
            data.push_str(if index == 0 { "M " } else { "L " });
            data.push_str(&format!("{x:.6},{y:.6} "));
        }
        data.push_str("Z ");
    }

    let path_element = svg::node::element::Path::new()
        .set("d", data)
        .set("fill", "#d3d3d3")
        .set("fill-rule", "evenodd")
        .set("stroke", "black")
        .set("vector-effect", "non-scaling-stroke");
    let document = svg::Document::new()
        .set("width", "600")
        .set("height", "600")
        .set(
            "viewBox",
            format!("{view_min_x} {view_min_y} {view_width} {view_height}"),
        )
        .add(
            svg::node::element::Group::new()
                .set(
                    "transform",
                    format!("translate(0 {}) scale(1 -1)", min_y + max_y),
                )
                .add(path_element),
        );
    std::fs::write(path, document.to_string()).map_err(SliceError::Io)
}

pub fn write_perimeter_layer_svg(
    layer: &PerimeterLayer,
    path: impl AsRef<Path>,
) -> Result<(), SliceError> {
    let coordinates = layer
        .walls
        .iter()
        .flat_map(|wall| wall.polygon.points.iter())
        .map(|point| {
            (
                point.x as f64 / DEFAULT_SCALE,
                point.y as f64 / DEFAULT_SCALE,
            )
        })
        .collect::<Vec<_>>();
    let (min_x, max_x, min_y, max_y) = coordinates.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), &(x, y)| {
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    );
    let (min_x, max_x, min_y, max_y) = if coordinates.is_empty() {
        (0.0, 1.0, 0.0, 1.0)
    } else {
        (min_x, max_x, min_y, max_y)
    };
    let margin = 1.0;
    let view_min_x = min_x - margin;
    let view_min_y = min_y - margin;
    let view_width = (max_x - min_x + 2.0 * margin).max(1.0);
    let view_height = (max_y - min_y + 2.0 * margin).max(1.0);
    let colors = [
        "#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00", "#a65628",
    ];
    let mut group = svg::node::element::Group::new().set(
        "transform",
        format!("translate(0 {}) scale(1 -1)", min_y + max_y),
    );
    for wall in &layer.walls {
        let mut data = String::new();
        for (index, point) in wall.polygon.points.iter().enumerate() {
            let x = point.x as f64 / DEFAULT_SCALE;
            let y = point.y as f64 / DEFAULT_SCALE;
            data.push_str(if index == 0 { "M " } else { "L " });
            data.push_str(&format!("{x:.6},{y:.6} "));
        }
        data.push('Z');
        let path_element = svg::node::element::Path::new()
            .set("d", data)
            .set("fill", "none")
            .set("stroke", colors[wall.wall_index % colors.len()])
            .set("stroke-width", "0.08")
            .set("vector-effect", "non-scaling-stroke");
        group = group.add(path_element);
    }
    let document = svg::Document::new()
        .set("width", "600")
        .set("height", "600")
        .set(
            "viewBox",
            format!("{view_min_x} {view_min_y} {view_width} {view_height}"),
        )
        .add(group);
    std::fs::write(path, document.to_string()).map_err(SliceError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Mesh {
        let vertices = vec![
            Vertex3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vertex3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vertex3 {
                x: 1.0,
                y: 1.0,
                z: 0.0,
            },
            Vertex3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vertex3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            Vertex3 {
                x: 1.0,
                y: 0.0,
                z: 1.0,
            },
            Vertex3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            Vertex3 {
                x: 0.0,
                y: 1.0,
                z: 1.0,
            },
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ]
        .into_iter()
        .map(|vertices| Triangle { vertices })
        .collect();
        Mesh {
            vertices,
            triangles,
        }
    }

    #[test]
    fn slices_cube_into_closed_contour() {
        let layers = slice_mesh(
            &cube(),
            SliceConfig {
                layer_height: 0.5,
                first_layer_height: 0.5,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].contours.len(), 1);
        assert!(layers[0].contours[0].is_closed());
        assert_eq!(layers[0].contours[0].signed_area().abs(), 1_000_000);
    }

    #[test]
    fn rejects_degenerate_triangles() {
        let mesh = Mesh {
            vertices: vec![
                Vertex3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vertex3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vertex3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            triangles: vec![Triangle {
                vertices: [0, 1, 2],
            }],
        };
        assert!(matches!(
            mesh.validate(),
            Err(SliceError::DegenerateTriangle(0))
        ));
    }

    #[test]
    fn assigns_horizontal_edges_to_the_upper_side_of_the_plane() {
        let layers = slice_mesh(
            &cube(),
            SliceConfig {
                layer_height: 1.0,
                first_layer_height: 1.0,
                ..SliceConfig::default()
            },
        )
        .unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].contours.len(), 1);
        assert_eq!(layers[0].contours[0].signed_area().abs(), 1_000_000);
    }

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
    fn generates_three_nested_cube_walls_with_half_width_start() {
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![square(0, 20_000)],
        }];
        let perimeters = generate_perimeters(
            &layers,
            PerimeterConfig {
                wall_count: 3,
                line_width: 0.4,
                nozzle_diameter: 0.4,
            },
        )
        .unwrap();
        assert_eq!(perimeters[0].walls.len(), 3);
        assert_eq!(
            bounds(&perimeters[0].walls[0].polygon),
            (200, 19_800, 200, 19_800)
        );
        assert_eq!(
            bounds(&perimeters[0].walls[1].polygon),
            (600, 19_400, 600, 19_400)
        );
        assert_eq!(
            bounds(&perimeters[0].walls[2].polygon),
            (1_000, 19_000, 1_000, 19_000)
        );
    }

    #[test]
    fn generates_walls_on_both_sides_of_a_ring() {
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![square(0, 20_000), square(5_000, 15_000)],
        }];
        let perimeters = generate_perimeters(
            &layers,
            PerimeterConfig {
                wall_count: 2,
                line_width: 0.4,
                nozzle_diameter: 0.4,
            },
        )
        .unwrap();
        assert_eq!(perimeters[0].walls.len(), 4);
        let outer_walls = perimeters[0]
            .walls
            .iter()
            .filter(|wall| !wall.is_hole)
            .collect::<Vec<_>>();
        let hole_walls = perimeters[0]
            .walls
            .iter()
            .filter(|wall| wall.is_hole)
            .collect::<Vec<_>>();
        assert_eq!(outer_walls.len(), 2);
        assert_eq!(hole_walls.len(), 2);
        assert_eq!(
            bounds(&hole_walls[0].polygon),
            (4_800, 15_200, 4_800, 15_200)
        );
        assert_eq!(
            bounds(&hole_walls[1].polygon),
            (4_400, 15_600, 4_400, 15_600)
        );
    }

    #[test]
    fn removes_collinear_outline_points() {
        let polygon = Polygon {
            points: vec![
                Point2 { x: 0, y: 0 },
                Point2 { x: 5, y: 0 },
                Point2 { x: 10, y: 0 },
                Point2 { x: 10, y: 10 },
                Point2 { x: 0, y: 10 },
            ],
        };
        assert_eq!(simplify_polygon(&polygon).points, square(0, 10).points);
    }
}
