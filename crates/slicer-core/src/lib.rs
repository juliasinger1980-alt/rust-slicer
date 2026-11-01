use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};

use rayon::prelude::*;

pub const DEFAULT_SCALE: f64 = 1_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub vertices: [usize; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex3>,
    pub triangles: Vec<Triangle>,
}

impl Mesh {
    pub fn from_stl(path: impl AsRef<Path>) -> Result<Self, SliceError> {
        let file = File::open(path).map_err(SliceError::Io)?;
        Self::from_stl_reader(BufReader::new(file))
    }

    pub fn from_stl_reader(mut reader: impl Read) -> Result<Self, SliceError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(SliceError::Io)?;
        let stl = stl_io::read_stl(&mut Cursor::new(bytes))
            .map_err(|error| SliceError::InvalidStl(error.to_string()))?;

        let vertices = stl
            .vertices
            .into_iter()
            .map(|vertex| Vertex3 {
                x: f64::from(vertex[0]),
                y: f64::from(vertex[1]),
                z: f64::from(vertex[2]),
            })
            .collect();
        let triangles = stl
            .faces
            .into_iter()
            .map(|face| Triangle {
                vertices: face.vertices,
            })
            .collect();
        let mesh = Self {
            vertices,
            triangles,
        };
        mesh.validate()?;
        Ok(mesh)
    }

    pub fn validate(&self) -> Result<(), SliceError> {
        if self.vertices.is_empty() || self.triangles.is_empty() {
            return Err(SliceError::EmptyMesh);
        }
        for (index, vertex) in self.vertices.iter().enumerate() {
            if !vertex.x.is_finite() || !vertex.y.is_finite() || !vertex.z.is_finite() {
                return Err(SliceError::NonFiniteVertex(index));
            }
        }
        for (index, triangle) in self.triangles.iter().enumerate() {
            if triangle
                .vertices
                .iter()
                .any(|&vertex| vertex >= self.vertices.len())
            {
                return Err(SliceError::InvalidTriangle(index));
            }
            let [a, b, c] = triangle.vertices;
            let ab = self.vertices[b];
            let aa = self.vertices[a];
            let ac = self.vertices[c];
            let cross = (ab.x - aa.x) * (ac.y - aa.y) - (ab.y - aa.y) * (ac.x - aa.x);
            let area_3d = cross.abs()
                + ((ab.x - aa.x) * (ac.z - aa.z) - (ab.z - aa.z) * (ac.x - aa.x)).abs()
                + ((ab.y - aa.y) * (ac.z - aa.z) - (ab.z - aa.z) * (ac.y - aa.y)).abs();
            if area_3d <= f64::EPSILON {
                return Err(SliceError::DegenerateTriangle(index));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Point2 {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    pub points: Vec<Point2>,
}

impl Polygon {
    pub fn signed_area(&self) -> i128 {
        self.points
            .iter()
            .zip(self.points.iter().cycle().skip(1))
            .take(self.points.len())
            .map(|(a, b)| i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y))
            .sum::<i128>()
            / 2
    }

    pub fn is_closed(&self) -> bool {
        self.points.len() >= 3 && self.signed_area() != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub z: f64,
    pub contours: Vec<Polygon>,
}

#[derive(Clone, Copy, Debug)]
pub struct SliceConfig {
    pub layer_height: f64,
    pub first_layer_height: f64,
    pub scale: f64,
}

impl Default for SliceConfig {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            first_layer_height: 0.2,
            scale: DEFAULT_SCALE,
        }
    }
}

impl SliceConfig {
    fn validate(self) -> Result<Self, SliceError> {
        if !self.layer_height.is_finite()
            || !self.first_layer_height.is_finite()
            || !self.scale.is_finite()
            || self.layer_height <= 0.0
            || self.first_layer_height <= 0.0
            || self.scale <= 0.0
        {
            return Err(SliceError::InvalidConfiguration);
        }
        Ok(self)
    }
}

#[derive(Debug)]
pub enum SliceError {
    Io(std::io::Error),
    InvalidStl(String),
    EmptyMesh,
    NonFiniteVertex(usize),
    InvalidTriangle(usize),
    DegenerateTriangle(usize),
    InvalidConfiguration,
}

impl fmt::Display for SliceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read STL: {error}"),
            Self::InvalidStl(error) => write!(formatter, "invalid STL: {error}"),
            Self::EmptyMesh => write!(formatter, "mesh has no vertices or triangles"),
            Self::NonFiniteVertex(index) => write!(formatter, "vertex {index} is not finite"),
            Self::InvalidTriangle(index) => {
                write!(formatter, "triangle {index} has an invalid index")
            }
            Self::DegenerateTriangle(index) => write!(formatter, "triangle {index} has zero area"),
            Self::InvalidConfiguration => write!(formatter, "invalid slicing configuration"),
        }
    }
}

impl std::error::Error for SliceError {}

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
}
