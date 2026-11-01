use std::{
    fmt,
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerimeterConfig {
    pub wall_count: usize,
    pub line_width: f64,
    pub nozzle_diameter: f64,
}

impl Default for PerimeterConfig {
    fn default() -> Self {
        Self {
            wall_count: 3,
            line_width: 0.4,
            nozzle_diameter: 0.4,
        }
    }
}

impl PerimeterConfig {
    pub(crate) fn validate(self) -> Result<Self, SliceError> {
        if !self.line_width.is_finite()
            || !self.nozzle_diameter.is_finite()
            || self.line_width <= 0.0
            || self.nozzle_diameter <= 0.0
        {
            return Err(SliceError::InvalidPerimeterConfiguration);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WallLoop {
    pub wall_index: usize,
    pub is_hole: bool,
    pub polygon: Polygon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PerimeterLayer {
    pub z: f64,
    pub walls: Vec<WallLoop>,
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
            scale: crate::DEFAULT_SCALE,
        }
    }
}

impl SliceConfig {
    pub(crate) fn validate(self) -> Result<Self, SliceError> {
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
    InvalidPerimeterConfiguration,
    InvalidInfillConfiguration,
    BooleanOperation(String),
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
            Self::InvalidPerimeterConfiguration => {
                write!(formatter, "invalid perimeter configuration")
            }
            Self::InvalidInfillConfiguration => write!(formatter, "invalid infill configuration"),
            Self::BooleanOperation(error) => {
                write!(formatter, "polygon boolean operation failed: {error}")
            }
        }
    }
}

impl std::error::Error for SliceError {}
