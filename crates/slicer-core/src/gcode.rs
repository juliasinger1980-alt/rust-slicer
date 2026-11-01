use std::{collections::HashMap, fmt, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    AdhesionConfig, AdhesionGeometry, CoolingConfig, DEFAULT_SCALE, InfillLayer, Layer,
    PerimeterLayer, Point2, SolidInfillLayer, generate_adhesion,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct GCodeConfig {
    pub nozzle_diameter: f64,
    pub filament_diameter: f64,
    pub filament_density_g_cm3: f64,
    pub layer_height: f64,
    pub first_layer_height: f64,
    pub nozzle_temperature: u16,
    pub first_layer_nozzle_temperature: u16,
    pub bed_temperature: u16,
    pub first_layer_bed_temperature: u16,
    pub bed_width: f64,
    pub bed_depth: f64,
    pub bed_max_z: f64,
    pub outer_wall_speed: f64,
    pub inner_wall_speed: f64,
    pub solid_infill_speed: f64,
    pub sparse_infill_speed: f64,
    pub travel_speed: f64,
    pub first_layer_speed: f64,
    pub first_layer_travel_speed: f64,
    pub retraction_distance: f64,
    pub retraction_speed: f64,
    pub minimum_retraction_travel: f64,
    pub seam_position: SeamPosition,
    pub travel_optimization: bool,
    pub avoid_crossing_perimeters: bool,
    pub fan_speed: u8,
    pub first_layer_fan_speed: u8,
    pub maximum_speed: f64,
    pub maximum_temperature: u16,
    pub start_gcode: String,
    pub end_gcode: String,
}

impl Default for GCodeConfig {
    fn default() -> Self {
        Self {
            nozzle_diameter: 0.4,
            filament_diameter: 1.75,
            filament_density_g_cm3: 1.24,
            layer_height: 0.2,
            first_layer_height: 0.2,
            nozzle_temperature: 200,
            first_layer_nozzle_temperature: 205,
            bed_temperature: 60,
            first_layer_bed_temperature: 60,
            bed_width: 220.0,
            bed_depth: 220.0,
            bed_max_z: 250.0,
            outer_wall_speed: 40.0,
            inner_wall_speed: 80.0,
            solid_infill_speed: 60.0,
            sparse_infill_speed: 80.0,
            travel_speed: 150.0,
            first_layer_speed: 20.0,
            first_layer_travel_speed: 80.0,
            retraction_distance: 0.8,
            retraction_speed: 35.0,
            minimum_retraction_travel: 1.0,
            seam_position: SeamPosition::Aligned,
            travel_optimization: true,
            avoid_crossing_perimeters: true,
            fan_speed: 128,
            first_layer_fan_speed: 0,
            maximum_speed: 300.0,
            maximum_temperature: 300,
            start_gcode: "M104 S{nozzle_temperature}\nM140 S{bed_temperature}\nM109 S{nozzle_temperature}\nM190 S{bed_temperature}".into(),
            end_gcode: "M104 S0\nM140 S0\nM106 S0\nG1 X0 Y0 F6000".into(),
        }
    }
}

impl GCodeConfig {
    fn validate(&self) -> Result<(), GCodeError> {
        let positive = [
            self.nozzle_diameter,
            self.filament_diameter,
            self.filament_density_g_cm3,
            self.layer_height,
            self.first_layer_height,
            self.bed_width,
            self.bed_depth,
            self.bed_max_z,
            self.outer_wall_speed,
            self.inner_wall_speed,
            self.solid_infill_speed,
            self.sparse_infill_speed,
            self.travel_speed,
            self.first_layer_speed,
            self.first_layer_travel_speed,
            self.retraction_speed,
            self.maximum_speed,
        ];
        if positive
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || !self.retraction_distance.is_finite()
            || self.retraction_distance < 0.0
            || !self.minimum_retraction_travel.is_finite()
            || self.minimum_retraction_travel < 0.0
            || self.maximum_temperature == 0
        {
            return Err(GCodeError::InvalidConfiguration);
        }
        Ok(())
    }
}

pub fn load_gcode_config(path: impl AsRef<Path>) -> Result<GCodeConfig, GCodeError> {
    let text = fs::read_to_string(path).map_err(GCodeError::Io)?;
    let config = toml::from_str::<GCodeConfig>(&text)
        .map_err(|error| GCodeError::Config(error.to_string()))?;
    config.validate()?;
    Ok(config)
}

pub fn extrusion_for_length(
    length: f64,
    line_width: f64,
    layer_height: f64,
    filament_diameter: f64,
) -> f64 {
    let filament_area = std::f64::consts::PI * filament_diameter.powi(2) / 4.0;
    length * line_width * layer_height / filament_area
}

pub fn generate_gcode(
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    config: &GCodeConfig,
) -> Result<String, GCodeError> {
    Ok(generate_gcode_with_report(perimeters, solid, sparse, config)?.gcode)
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeamPosition {
    Nearest,
    Aligned,
    Rear,
    Random,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GCodeReport {
    pub gcode: String,
    pub total_travel_distance: f64,
    pub estimated_print_time_seconds: f64,
    pub cooling_unreachable_layers: usize,
    pub total_extrusion_mm: f64,
}

pub fn generate_gcode_with_report(
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    config: &GCodeConfig,
) -> Result<GCodeReport, GCodeError> {
    generate_gcode_with_report_and_settings(
        perimeters,
        solid,
        sparse,
        None,
        &AdhesionConfig::default(),
        &CoolingConfig::default(),
        config,
    )
}

pub fn generate_gcode_with_report_and_adhesion(
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    layers: Option<&[Layer]>,
    adhesion_config: &AdhesionConfig,
    config: &GCodeConfig,
) -> Result<GCodeReport, GCodeError> {
    generate_gcode_with_report_and_settings(
        perimeters,
        solid,
        sparse,
        layers,
        adhesion_config,
        &CoolingConfig::default(),
        config,
    )
}

pub fn generate_gcode_with_report_and_settings(
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    layers: Option<&[Layer]>,
    adhesion_config: &AdhesionConfig,
    cooling_config: &CoolingConfig,
    config: &GCodeConfig,
) -> Result<GCodeReport, GCodeError> {
    config.validate()?;
    adhesion_config
        .validate()
        .map_err(|error| GCodeError::Adhesion(error.to_string()))?;
    cooling_config
        .validate()
        .map_err(|error| GCodeError::Cooling(error.to_string()))?;
    if perimeters.len() != solid.len() || perimeters.len() != sparse.len() {
        return Err(GCodeError::LayerCountMismatch);
    }
    let adhesion = if adhesion_config.skirt_loops > 0
        || (adhesion_config.brim_enabled && adhesion_config.brim_width > 0.0)
    {
        let first_layer = layers
            .and_then(|layers| layers.first())
            .ok_or(GCodeError::MissingFirstLayerOutlines)?;
        generate_adhesion(first_layer, *adhesion_config, config.nozzle_diameter)
            .map_err(|error| GCodeError::Adhesion(error.to_string()))?
    } else {
        AdhesionGeometry::default()
    };
    let (min_x, max_x, min_y, max_y) = bounds(perimeters, solid, sparse, &adhesion)?;
    let offset_x = (config.bed_width - (max_x - min_x)) * 0.5 - min_x;
    let offset_y = (config.bed_depth - (max_y - min_y)) * 0.5 - min_y;
    let mut output = String::from("; generated by rust-slicer\nG90\nM83\n");
    output.push_str(&expand_start_gcode(&config.start_gcode, config, true));
    output.push('\n');
    let mut position = None;
    let mut previous_seams = HashMap::new();
    let mut report = GCodeReport {
        gcode: String::new(),
        total_travel_distance: 0.0,
        estimated_print_time_seconds: 0.0,
        cooling_unreachable_layers: 0,
        total_extrusion_mm: 0.0,
    };
    for (index, ((perimeter, solid_layer), sparse_layer)) in
        perimeters.iter().zip(solid).zip(sparse).enumerate()
    {
        let first = index == 0;
        let z = if first {
            config.first_layer_height
        } else {
            perimeter.z
        };
        if z < -1e-6 || z > config.bed_max_z + 1e-6 {
            return Err(GCodeError::CoordinateOutsideBed { x: 0.0, y: 0.0, z });
        }
        output.push_str(&format!(";LAYER_CHANGE\n;Z:{z:.3}\n"));
        let nozzle_temperature = if first {
            config.first_layer_nozzle_temperature
        } else {
            config.nozzle_temperature
        }
        .min(config.maximum_temperature);
        let bed_temperature = if first {
            config.first_layer_bed_temperature
        } else {
            config.bed_temperature
        }
        .min(config.maximum_temperature);
        output.push_str(&format!(
            "M104 S{nozzle_temperature}\nM140 S{bed_temperature}\n"
        ));
        let fan_speed = layer_fan_speed(index, cooling_config, config);
        output.push_str(&format!("M106 S{fan_speed}\n"));
        let mut paths = plan_layer(
            perimeter,
            solid_layer,
            sparse_layer,
            position,
            &previous_seams,
            config,
            offset_x,
            offset_y,
            first,
            cooling_config,
        );
        if first {
            let mut adhesion_paths = plan_adhesion(&adhesion, config, position, offset_x, offset_y);
            adhesion_paths.append(&mut paths);
            paths = adhesion_paths;
        }
        if cooling_config.enabled && cool_paths(&mut paths, cooling_config, config.maximum_speed) {
            report.cooling_unreachable_layers += 1;
        }
        for path in &paths {
            if let (Some(wall_index), Some(&seam)) = (path.wall_index, path.points.first()) {
                previous_seams.insert(wall_index, seam);
            }
        }
        let walls = perimeter
            .walls
            .iter()
            .map(|wall| wall.polygon.points.as_slice())
            .collect::<Vec<_>>();
        for path in paths {
            emit_path(
                &mut output,
                &mut position,
                &path,
                z,
                &walls,
                config,
                offset_x,
                offset_y,
                first,
                &mut report,
            )?;
        }
    }
    output.push_str(&config.end_gcode);
    output.push('\n');
    report.gcode = output;
    Ok(report)
}

pub fn write_gcode(
    path: impl AsRef<Path>,
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    config: &GCodeConfig,
) -> Result<(), GCodeError> {
    let output = generate_gcode(perimeters, solid, sparse, config)?;
    fs::write(path, output).map_err(GCodeError::Io)
}

#[derive(Clone, Debug)]
struct PlannedPath {
    points: Vec<Point2>,
    closed: bool,
    wall_index: Option<usize>,
    role: &'static str,
    speed: f64,
}

#[allow(clippy::too_many_arguments)]
fn plan_layer(
    perimeter: &PerimeterLayer,
    solid: &SolidInfillLayer,
    sparse: &InfillLayer,
    current: Option<(f64, f64)>,
    previous_seams: &HashMap<usize, Point2>,
    config: &GCodeConfig,
    offset_x: f64,
    offset_y: f64,
    first: bool,
    cooling: &CoolingConfig,
) -> Vec<PlannedPath> {
    let current_local = current.map(|(x, y)| (x - offset_x, y - offset_y));
    let mut inner_paths = Vec::new();
    for wall in perimeter.walls.iter().filter(|wall| wall.wall_index > 0) {
        inner_paths.push(PlannedPath {
            points: seam_points(
                &wall.polygon.points,
                current_local,
                previous_seams.get(&wall.wall_index).copied(),
                config,
            ),
            closed: true,
            wall_index: Some(wall.wall_index),
            role: "Inner wall",
            speed: selected_wall_speed(
                config.inner_wall_speed,
                &wall.polygon.points,
                first,
                cooling,
                config,
            ),
        });
    }

    let mut paths = order_paths(inner_paths, current_local, config.travel_optimization);
    let mut outer_paths = Vec::new();
    for wall in perimeter.walls.iter().filter(|wall| wall.wall_index == 0) {
        outer_paths.push(PlannedPath {
            points: seam_points(
                &wall.polygon.points,
                current_local,
                previous_seams.get(&wall.wall_index).copied(),
                config,
            ),
            closed: true,
            wall_index: Some(wall.wall_index),
            role: "Outer wall",
            speed: selected_wall_speed(
                config.outer_wall_speed,
                &wall.polygon.points,
                first,
                cooling,
                config,
            ),
        });
    }
    paths.extend(order_paths(
        outer_paths,
        current_local,
        config.travel_optimization,
    ));
    paths.extend(order_lines(
        &solid.solid_lines,
        "Solid infill",
        config.solid_infill_speed,
        current_local,
        config.travel_optimization,
    ));
    paths.extend(order_lines(
        &sparse.lines,
        "Sparse infill",
        config.sparse_infill_speed,
        current_local,
        config.travel_optimization,
    ));
    paths
}

fn selected_wall_speed(
    normal_speed: f64,
    points: &[Point2],
    first: bool,
    cooling: &CoolingConfig,
    config: &GCodeConfig,
) -> f64 {
    let speed = if !first
        && cooling.enabled
        && cooling.small_perimeter_threshold_mm > 0.0
        && polygon_length(points) < cooling.small_perimeter_threshold_mm
    {
        cooling.small_perimeter_speed_mm_s
    } else {
        normal_speed
    };
    speed.min(config.maximum_speed)
}

fn polygon_length(points: &[Point2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| {
            ((b.x - a.x) as f64 / DEFAULT_SCALE).hypot((b.y - a.y) as f64 / DEFAULT_SCALE)
        })
        .sum()
}

fn cool_paths(paths: &mut [PlannedPath], cooling: &CoolingConfig, maximum_speed: f64) -> bool {
    for path in paths.iter_mut() {
        path.speed = path.speed.min(maximum_speed);
    }
    if cooling.minimum_layer_time_s <= 0.0 {
        return false;
    }
    let current_time = paths
        .iter()
        .map(|path| path_length(path) / path.speed)
        .sum::<f64>();
    if current_time <= f64::EPSILON || current_time >= cooling.minimum_layer_time_s {
        return false;
    }
    let minimum_speed = cooling.minimum_print_speed_mm_s.min(maximum_speed);
    let slowed_time = paths
        .iter()
        .map(|path| path_length(path) / minimum_speed)
        .sum::<f64>();
    let unreachable = slowed_time < cooling.minimum_layer_time_s;
    let factor = cooling.minimum_layer_time_s / current_time;
    for path in paths {
        path.speed = (path.speed / factor).max(minimum_speed).min(maximum_speed);
    }
    unreachable
}

fn path_length(path: &PlannedPath) -> f64 {
    if path.closed {
        polygon_length(&path.points)
    } else {
        path.points
            .windows(2)
            .map(|pair| {
                ((pair[1].x - pair[0].x) as f64 / DEFAULT_SCALE)
                    .hypot((pair[1].y - pair[0].y) as f64 / DEFAULT_SCALE)
            })
            .sum()
    }
}

fn layer_fan_speed(layer: usize, cooling: &CoolingConfig, config: &GCodeConfig) -> u8 {
    if !cooling.enabled || !cooling.fan_enabled {
        return if layer == 0 {
            config.first_layer_fan_speed
        } else {
            config.fan_speed
        };
    }
    if layer < cooling.fan_off_first_layers {
        return 0;
    }
    if cooling.fan_full_speed_layer > cooling.fan_off_first_layers
        && layer < cooling.fan_full_speed_layer
    {
        let numerator = layer.saturating_sub(cooling.fan_off_first_layers) as f64;
        let denominator = (cooling.fan_full_speed_layer - cooling.fan_off_first_layers) as f64;
        let ratio = (numerator / denominator).clamp(0.0, 1.0);
        return (f64::from(cooling.fan_min_speed)
            + ratio * f64::from(cooling.fan_max_speed - cooling.fan_min_speed))
        .round() as u8;
    }
    cooling.fan_max_speed
}

fn plan_adhesion(
    adhesion: &AdhesionGeometry,
    config: &GCodeConfig,
    current: Option<(f64, f64)>,
    offset_x: f64,
    offset_y: f64,
) -> Vec<PlannedPath> {
    let current_local = current.map(|(x, y)| (x - offset_x, y - offset_y));
    let skirt_paths = adhesion
        .skirts
        .iter()
        .map(|polygon| PlannedPath {
            points: seam_points(&polygon.points, current_local, None, config),
            closed: true,
            wall_index: None,
            role: "Skirt",
            speed: config.first_layer_speed,
        })
        .collect::<Vec<_>>();
    let brim_paths = adhesion
        .brims
        .iter()
        .map(|polygon| PlannedPath {
            points: seam_points(&polygon.points, current_local, None, config),
            closed: true,
            wall_index: None,
            role: "Brim",
            speed: config.first_layer_speed,
        })
        .collect::<Vec<_>>();
    let mut paths = order_paths(skirt_paths, current_local, config.travel_optimization);
    paths.extend(order_paths(
        brim_paths,
        current_local,
        config.travel_optimization,
    ));
    paths
}

fn order_paths(
    mut remaining: Vec<PlannedPath>,
    current: Option<(f64, f64)>,
    optimize: bool,
) -> Vec<PlannedPath> {
    if !optimize {
        return remaining;
    }
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut at = current;
    while !remaining.is_empty() {
        let index = remaining
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let a_distance = at.map_or(0.0, |at| point_distance_sq(a.points[0], at));
                let b_distance = at.map_or(0.0, |at| point_distance_sq(b.points[0], at));
                a_distance.total_cmp(&b_distance)
            })
            .map_or(0, |(index, _)| index);
        let path = remaining.swap_remove(index);
        at = Some(point_mm(path.points[0]));
        ordered.push(path);
    }
    ordered
}

fn seam_points(
    points: &[Point2],
    current: Option<(f64, f64)>,
    previous_seam: Option<Point2>,
    config: &GCodeConfig,
) -> Vec<Point2> {
    if points.is_empty() {
        return Vec::new();
    }
    let target = match config.seam_position {
        SeamPosition::Nearest => current,
        SeamPosition::Aligned => previous_seam
            .map(|point| {
                (
                    point.x as f64 / DEFAULT_SCALE,
                    point.y as f64 / DEFAULT_SCALE,
                )
            })
            .or(current),
        SeamPosition::Rear | SeamPosition::Random => None,
    };
    let index = match config.seam_position {
        SeamPosition::Rear => points
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.y.cmp(&b.y).then_with(|| a.x.cmp(&b.x)))
            .map_or(0, |(index, _)| index),
        SeamPosition::Random => {
            let seed = points.iter().fold(0_u64, |seed, point| {
                seed.wrapping_mul(1_664_525)
                    .wrapping_add(point.x as u64)
                    .wrapping_add(point.y as u64)
            });
            (seed as usize) % points.len()
        }
        SeamPosition::Nearest | SeamPosition::Aligned => target.map_or(0, |target| {
            points
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    point_distance_sq(**a, target).total_cmp(&point_distance_sq(**b, target))
                })
                .map_or(0, |(index, _)| index)
        }),
    };
    points
        .iter()
        .copied()
        .cycle()
        .skip(index)
        .take(points.len())
        .collect()
}

fn order_lines(
    lines: &[crate::InfillLine],
    role: &'static str,
    speed: f64,
    current: Option<(f64, f64)>,
    optimize: bool,
) -> Vec<PlannedPath> {
    let mut remaining = lines
        .iter()
        .map(|line| PlannedPath {
            points: vec![line.start, line.end],
            closed: false,
            wall_index: None,
            role,
            speed,
        })
        .collect::<Vec<_>>();
    if !optimize {
        return remaining;
    }
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut at = current;
    while !remaining.is_empty() {
        let (index, reverse, _) = remaining
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let start_distance = at.map_or(0.0, |at| point_distance_sq(path.points[0], at));
                let end_distance = at.map_or(0.0, |at| point_distance_sq(path.points[1], at));
                (
                    index,
                    end_distance < start_distance,
                    start_distance.min(end_distance),
                )
            })
            .min_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
            .unwrap();
        let mut path = remaining.swap_remove(index);
        if reverse {
            path.points.reverse();
        }
        at = Some(point_mm(path.points[1]));
        ordered.push(path);
    }
    ordered
}

fn point_mm(point: Point2) -> (f64, f64) {
    (
        point.x as f64 / DEFAULT_SCALE,
        point.y as f64 / DEFAULT_SCALE,
    )
}

fn point_distance_sq(point: Point2, target: (f64, f64)) -> f64 {
    let (x, y) = point_mm(point);
    (x - target.0).powi(2) + (y - target.1).powi(2)
}

#[allow(clippy::too_many_arguments)]
fn emit_path(
    output: &mut String,
    position: &mut Option<(f64, f64)>,
    path: &PlannedPath,
    z: f64,
    walls: &[&[Point2]],
    config: &GCodeConfig,
    offset_x: f64,
    offset_y: f64,
    first: bool,
    report: &mut GCodeReport,
) -> Result<(), GCodeError> {
    if path.points.len() < if path.closed { 3 } else { 2 } {
        return Ok(());
    }
    let start = path.points[0];
    emit_travel(
        output, position, start, z, walls, config, offset_x, offset_y, first, report,
    )?;
    let points = path
        .points
        .iter()
        .copied()
        .skip(1)
        .chain(path.closed.then_some(path.points[0]));
    let mut previous = start;
    for point in points {
        emit_extrusion(
            output, position, previous, point, z, path, config, offset_x, offset_y, first, report,
        )?;
        previous = point;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_travel(
    output: &mut String,
    position: &mut Option<(f64, f64)>,
    point: Point2,
    z: f64,
    walls: &[&[Point2]],
    config: &GCodeConfig,
    offset_x: f64,
    offset_y: f64,
    first: bool,
    report: &mut GCodeReport,
) -> Result<(), GCodeError> {
    let (x, y) = translate(point, offset_x, offset_y);
    check_coordinate(x, y, z, config)?;
    if position.is_some_and(|current| current != (x, y)) {
        let current = position.unwrap();
        let route = travel_route(current, (x, y), walls, config.avoid_crossing_perimeters)?;
        let route_distance = route
            .windows(2)
            .map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1))
            .sum::<f64>();
        report.total_travel_distance += route_distance;
        report.estimated_print_time_seconds += route_distance / config.travel_speed / 60.0;
        if route_distance > config.minimum_retraction_travel && config.retraction_distance > 0.0 {
            output.push_str(&format!(
                "G1 E-{:.5} F{}\n",
                config.retraction_distance,
                feedrate(config.retraction_speed, config)
            ));
        }
        for (route_x, route_y) in route.into_iter().skip(1) {
            output.push_str(&format!(
                "G0 X{route_x:.3} Y{route_y:.3} Z{z:.3} F{}\n",
                feedrate(
                    if first {
                        config.first_layer_travel_speed
                    } else {
                        config.travel_speed
                    },
                    config
                )
            ));
        }
        if route_distance > config.minimum_retraction_travel && config.retraction_distance > 0.0 {
            output.push_str(&format!(
                "G1 E{:.5} F{}\n",
                config.retraction_distance,
                feedrate(config.retraction_speed, config)
            ));
        }
    } else if position.is_none() {
        output.push_str(&format!(
            ";TYPE:{}\nG0 X{x:.3} Y{y:.3} Z{z:.3} F{}\n",
            "travel",
            feedrate(
                if first {
                    config.first_layer_travel_speed
                } else {
                    config.travel_speed
                },
                config
            )
        ));
    }
    *position = Some((x, y));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_extrusion(
    output: &mut String,
    position: &mut Option<(f64, f64)>,
    previous: Point2,
    point: Point2,
    z: f64,
    path: &PlannedPath,
    config: &GCodeConfig,
    offset_x: f64,
    offset_y: f64,
    first: bool,
    report: &mut GCodeReport,
) -> Result<(), GCodeError> {
    let (x, y) = translate(point, offset_x, offset_y);
    check_coordinate(x, y, z, config)?;
    let (previous_x, previous_y) = translate(previous, offset_x, offset_y);
    let length = (x - previous_x).hypot(y - previous_y);
    if length <= f64::EPSILON {
        return Ok(());
    }
    let e = extrusion_for_length(
        length,
        config.nozzle_diameter,
        if first {
            config.first_layer_height
        } else {
            config.layer_height
        },
        config.filament_diameter,
    );
    let speed = if first {
        config.first_layer_speed.min(config.maximum_speed)
    } else {
        path.speed.min(config.maximum_speed)
    };
    output.push_str(&format!(
        ";TYPE:{}\nG1 X{x:.3} Y{y:.3} Z{z:.3} E{e:.5} F{}\n",
        path.role,
        feedrate(speed, config)
    ));
    report.estimated_print_time_seconds += length / speed / 60.0;
    report.total_extrusion_mm += e;
    *position = Some((x, y));
    Ok(())
}

fn travel_route(
    start: (f64, f64),
    end: (f64, f64),
    walls: &[&[Point2]],
    avoid_crossing: bool,
) -> Result<Vec<(f64, f64)>, GCodeError> {
    if !avoid_crossing || !travel_crosses_walls(start, end, walls) {
        return Ok(vec![start, end]);
    }
    let mut candidates = Vec::new();
    for wall in walls {
        for point in *wall {
            let candidate = point_mm(*point);
            if !travel_crosses_walls(start, candidate, walls)
                && !travel_crosses_walls(candidate, end, walls)
            {
                let distance = (candidate.0 - start.0).hypot(candidate.1 - start.1)
                    + (end.0 - candidate.0).hypot(end.1 - candidate.1);
                candidates.push((distance, candidate));
            }
        }
    }
    if let Some((_, candidate)) = candidates.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
        return Ok(vec![start, candidate, end]);
    }

    let candidates: Vec<(f64, f64)> = walls
        .iter()
        .flat_map(|wall| wall.iter().copied().map(point_mm))
        .collect();
    let mut best = None;
    for &first in &candidates {
        if travel_crosses_walls(start, first, walls) {
            continue;
        }
        for &second in &candidates {
            if travel_crosses_walls(first, second, walls)
                || travel_crosses_walls(second, end, walls)
            {
                continue;
            }
            let distance = (first.0 - start.0).hypot(first.1 - start.1)
                + (second.0 - first.0).hypot(second.1 - first.1)
                + (end.0 - second.0).hypot(end.1 - second.1);
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, (first, second)));
            }
        }
    }
    if let Some((_, (first, second))) = best {
        return Ok(vec![start, first, second, end]);
    }

    Ok(vec![start, end])
}

fn travel_crosses_walls(start: (f64, f64), end: (f64, f64), walls: &[&[Point2]]) -> bool {
    walls.iter().any(|wall| {
        wall.iter()
            .copied()
            .zip(wall.iter().copied().cycle().skip(1))
            .take(wall.len())
            .any(|(a, b)| segments_intersect(start, end, point_mm(a), point_mm(b)))
    })
}

fn segments_intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    fn orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }
    fn same_point(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() <= 1e-9 && (a.1 - b.1).abs() <= 1e-9
    }
    fn on_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> bool {
        let cross =
            (end.0 - start.0) * (point.1 - start.1) - (end.1 - start.1) * (point.0 - start.0);
        cross.abs() <= 1e-8
            && point.0 >= start.0.min(end.0) - 1e-8
            && point.0 <= start.0.max(end.0) + 1e-8
            && point.1 >= start.1.min(end.1) - 1e-8
            && point.1 <= start.1.max(end.1) + 1e-8
    }
    if same_point(a, c)
        || same_point(a, d)
        || same_point(b, c)
        || same_point(b, d)
        || on_segment(a, c, d)
        || on_segment(b, c, d)
    {
        return false;
    }
    let ab_c = orientation(a, b, c);
    let ab_d = orientation(a, b, d);
    let cd_a = orientation(c, d, a);
    let cd_b = orientation(c, d, b);
    let opposite =
        |left: f64, right: f64| (left > 1e-9 && right < -1e-9) || (left < -1e-9 && right > 1e-9);
    opposite(ab_c, ab_d) && opposite(cd_a, cd_b)
}

fn feedrate(speed: f64, config: &GCodeConfig) -> u64 {
    (speed.min(config.maximum_speed) * 60.0).round() as u64
}

fn translate(point: Point2, offset_x: f64, offset_y: f64) -> (f64, f64) {
    (
        point.x as f64 / DEFAULT_SCALE + offset_x,
        point.y as f64 / DEFAULT_SCALE + offset_y,
    )
}

fn check_coordinate(x: f64, y: f64, z: f64, config: &GCodeConfig) -> Result<(), GCodeError> {
    if x < -1e-6
        || x > config.bed_width + 1e-6
        || y < -1e-6
        || y > config.bed_depth + 1e-6
        || z < -1e-6
        || z > config.bed_max_z + 1e-6
    {
        return Err(GCodeError::CoordinateOutsideBed { x, y, z });
    }
    Ok(())
}

fn bounds(
    perimeters: &[PerimeterLayer],
    solid: &[SolidInfillLayer],
    sparse: &[InfillLayer],
    adhesion: &AdhesionGeometry,
) -> Result<(f64, f64, f64, f64), GCodeError> {
    let mut bounds = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for wall in perimeters.iter().flat_map(|layer| layer.walls.iter()) {
        for point in &wall.polygon.points {
            update_bounds(&mut bounds, *point);
        }
    }
    for point in solid.iter().flat_map(|layer| {
        layer
            .solid_lines
            .iter()
            .flat_map(|line| [line.start, line.end])
    }) {
        update_bounds(&mut bounds, point);
    }
    for point in sparse
        .iter()
        .flat_map(|layer| layer.lines.iter().flat_map(|line| [line.start, line.end]))
    {
        update_bounds(&mut bounds, point);
    }
    for point in adhesion
        .skirts
        .iter()
        .chain(&adhesion.brims)
        .flat_map(|polygon| polygon.points.iter())
    {
        update_bounds(&mut bounds, *point);
    }
    if bounds.0.is_infinite() {
        Err(GCodeError::EmptyGeometry)
    } else {
        Ok(bounds)
    }
}

fn update_bounds(bounds: &mut (f64, f64, f64, f64), point: Point2) {
    let x = point.x as f64 / DEFAULT_SCALE;
    let y = point.y as f64 / DEFAULT_SCALE;
    bounds.0 = bounds.0.min(x);
    bounds.1 = bounds.1.max(x);
    bounds.2 = bounds.2.min(y);
    bounds.3 = bounds.3.max(y);
}

fn expand_start_gcode(template: &str, config: &GCodeConfig, first_layer: bool) -> String {
    let nozzle_temperature = if first_layer {
        config.first_layer_nozzle_temperature
    } else {
        config.nozzle_temperature
    }
    .min(config.maximum_temperature);
    let bed_temperature = if first_layer {
        config.first_layer_bed_temperature
    } else {
        config.bed_temperature
    }
    .min(config.maximum_temperature);
    template
        .replace("{nozzle_temperature}", &nozzle_temperature.to_string())
        .replace("{bed_temperature}", &bed_temperature.to_string())
}

#[derive(Debug)]
pub enum GCodeError {
    Io(std::io::Error),
    Config(String),
    InvalidConfiguration,
    LayerCountMismatch,
    EmptyGeometry,
    MissingFirstLayerOutlines,
    Adhesion(String),
    Cooling(String),
    CoordinateOutsideBed { x: f64, y: f64, z: f64 },
    NoSafeTravelRoute,
}

impl fmt::Display for GCodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "G-code I/O failed: {error}"),
            Self::Config(error) => write!(formatter, "invalid G-code TOML: {error}"),
            Self::InvalidConfiguration => write!(formatter, "invalid G-code configuration"),
            Self::LayerCountMismatch => {
                write!(formatter, "G-code inputs have different layer counts")
            }
            Self::EmptyGeometry => write!(formatter, "G-code inputs contain no printable geometry"),
            Self::MissingFirstLayerOutlines => {
                write!(formatter, "skirt or brim requires first-layer outlines")
            }
            Self::Adhesion(error) => write!(formatter, "failed to generate skirt or brim: {error}"),
            Self::Cooling(error) => write!(formatter, "cooling configuration failed: {error}"),
            Self::CoordinateOutsideBed { x, y, z } => write!(
                formatter,
                "coordinate ({x:.3}, {y:.3}, {z:.3}) is outside the bed volume"
            ),
            Self::NoSafeTravelRoute => {
                write!(formatter, "no safe perimeter-avoiding travel route exists")
            }
        }
    }
}

impl std::error::Error for GCodeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdhesionConfig, CoolingConfig, InfillLine, Layer, WallLoop};

    fn sample_inputs() -> (Vec<PerimeterLayer>, Vec<SolidInfillLayer>, Vec<InfillLayer>) {
        let wall = WallLoop {
            wall_index: 0,
            is_hole: false,
            polygon: crate::Polygon {
                points: vec![
                    Point2 {
                        x: 10_000,
                        y: 10_000,
                    },
                    Point2 {
                        x: 20_000,
                        y: 10_000,
                    },
                    Point2 {
                        x: 20_000,
                        y: 20_000,
                    },
                    Point2 {
                        x: 10_000,
                        y: 20_000,
                    },
                ],
            },
        };
        let perimeter = PerimeterLayer {
            z: 0.2,
            walls: vec![wall],
        };
        let line = InfillLine {
            start: Point2 {
                x: 12_000,
                y: 12_000,
            },
            end: Point2 {
                x: 18_000,
                y: 12_000,
            },
        };
        (
            vec![perimeter],
            vec![SolidInfillLayer {
                z: 0.2,
                sparse_region: Vec::new(),
                solid_region: Vec::new(),
                sparse_lines: Vec::new(),
                solid_lines: vec![line],
            }],
            vec![InfillLayer {
                z: 0.2,
                angle: 0.0,
                lines: vec![line],
            }],
        )
    }

    #[test]
    fn straight_line_extrusion_matches_volume_formula() {
        let actual = extrusion_for_length(10.0, 0.4, 0.2, 1.75);
        let expected = 10.0 * 0.4 * 0.2 / (std::f64::consts::PI * 1.75_f64.powi(2) / 4.0);
        assert!((actual - expected).abs() < 1e-12);
        assert_eq!(GCodeConfig::default().seam_position, SeamPosition::Aligned);
    }

    #[test]
    fn generated_layer_count_and_first_layer_settings_match_inputs() {
        let (perimeters, solid, sparse) = sample_inputs();
        let config = GCodeConfig {
            first_layer_speed: 17.0,
            first_layer_fan_speed: 33,
            ..Default::default()
        };
        let output = generate_gcode(&perimeters, &solid, &sparse, &config).unwrap();
        assert_eq!(output.matches(";LAYER_CHANGE").count(), perimeters.len());
        assert!(output.contains("M106 S33"));
        assert!(output.contains("F1020"));
    }

    #[test]
    fn short_layer_is_slowed_to_minimum_time_when_possible() {
        let mut paths = vec![PlannedPath {
            points: vec![Point2 { x: 0, y: 0 }, Point2 { x: 10_000, y: 0 }],
            closed: false,
            wall_index: None,
            role: "test",
            speed: 20.0,
        }];
        let unreachable = cool_paths(
            &mut paths,
            &CoolingConfig {
                enabled: true,
                minimum_layer_time_s: 1.0,
                minimum_print_speed_mm_s: 1.0,
                ..Default::default()
            },
            300.0,
        );
        assert!(!unreachable);
        assert!((paths[0].speed - 10.0).abs() < 1e-9);
    }

    #[test]
    fn fan_is_off_then_ramps_and_clamps() {
        let cooling = CoolingConfig {
            enabled: true,
            fan_enabled: true,
            fan_off_first_layers: 2,
            fan_full_speed_layer: 4,
            fan_min_speed: 20,
            fan_max_speed: 200,
            ..Default::default()
        };
        assert_eq!(layer_fan_speed(0, &cooling, &GCodeConfig::default()), 0);
        assert_eq!(layer_fan_speed(1, &cooling, &GCodeConfig::default()), 0);
        assert_eq!(layer_fan_speed(2, &cooling, &GCodeConfig::default()), 20);
        assert_eq!(layer_fan_speed(3, &cooling, &GCodeConfig::default()), 110);
        assert_eq!(layer_fan_speed(4, &cooling, &GCodeConfig::default()), 200);
    }

    #[test]
    fn cooling_does_not_change_extrusion_amount() {
        let (perimeters, solid, sparse) = sample_inputs();
        let normal = generate_gcode(&perimeters, &solid, &sparse, &GCodeConfig::default()).unwrap();
        let cooled = generate_gcode_with_report_and_settings(
            &perimeters,
            &solid,
            &sparse,
            None,
            &AdhesionConfig::default(),
            &CoolingConfig {
                enabled: true,
                minimum_layer_time_s: 10.0,
                minimum_print_speed_mm_s: 1.0,
                fan_enabled: true,
                ..Default::default()
            },
            &GCodeConfig::default(),
        )
        .unwrap()
        .gcode;
        let extrusion = |gcode: &str| {
            gcode
                .lines()
                .filter_map(|line| line.split(" E").nth(1))
                .filter_map(|value| value.split_whitespace().next())
                .filter_map(|value| value.parse::<f64>().ok())
                .filter(|value| *value > 0.0)
                .sum::<f64>()
        };
        assert!((extrusion(&normal) - extrusion(&cooled)).abs() < 1e-9);
    }

    #[test]
    fn adhesion_is_emitted_before_first_layer_walls() {
        let (perimeters, solid, sparse) = sample_inputs();
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![crate::Polygon {
                points: vec![
                    Point2 {
                        x: 10_000,
                        y: 10_000,
                    },
                    Point2 {
                        x: 20_000,
                        y: 10_000,
                    },
                    Point2 {
                        x: 20_000,
                        y: 20_000,
                    },
                    Point2 {
                        x: 10_000,
                        y: 20_000,
                    },
                ],
            }],
        }];
        let output = generate_gcode_with_report_and_adhesion(
            &perimeters,
            &solid,
            &sparse,
            Some(&layers),
            &AdhesionConfig {
                skirt_loops: 1,
                brim_enabled: true,
                brim_width: 0.4,
                ..Default::default()
            },
            &GCodeConfig::default(),
        )
        .unwrap()
        .gcode;
        let skirt = output.find(";TYPE:Skirt").unwrap();
        let brim = output.find(";TYPE:Brim").unwrap();
        let wall = output.find(";TYPE:Outer wall").unwrap();
        assert!(skirt < brim && brim < wall);
    }

    #[test]
    fn oversized_adhesion_is_rejected_by_bed_bounds() {
        let (perimeters, solid, sparse) = sample_inputs();
        let layers = vec![Layer {
            z: 0.2,
            contours: vec![crate::Polygon {
                points: vec![
                    Point2 { x: 0, y: 0 },
                    Point2 { x: 20_000, y: 0 },
                    Point2 {
                        x: 20_000,
                        y: 20_000,
                    },
                    Point2 { x: 0, y: 20_000 },
                ],
            }],
        }];
        let result = generate_gcode_with_report_and_adhesion(
            &perimeters,
            &solid,
            &sparse,
            Some(&layers),
            &AdhesionConfig {
                brim_enabled: true,
                brim_width: 1.0,
                ..Default::default()
            },
            &GCodeConfig {
                bed_width: 20.0,
                bed_depth: 20.0,
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(GCodeError::CoordinateOutsideBed { .. })
        ));
    }

    #[test]
    fn start_gcode_uses_clamped_first_layer_temperatures() {
        let (perimeters, solid, sparse) = sample_inputs();
        let config = GCodeConfig {
            first_layer_nozzle_temperature: 250,
            first_layer_bed_temperature: 90,
            maximum_temperature: 210,
            start_gcode: "M104 S{nozzle_temperature}\nM140 S{bed_temperature}".into(),
            ..Default::default()
        };
        let output = generate_gcode(&perimeters, &solid, &sparse, &config).unwrap();
        assert!(output.starts_with("; generated by rust-slicer\nG90\nM83\nM104 S210\nM140 S90"));
    }

    #[test]
    fn generated_coordinates_are_centered_inside_the_bed() {
        let (perimeters, solid, sparse) = sample_inputs();
        let config = GCodeConfig {
            bed_width: 20.0,
            bed_depth: 20.0,
            ..Default::default()
        };
        let output = generate_gcode(&perimeters, &solid, &sparse, &config).unwrap();
        assert!(output.lines().any(|line| line.contains("X5.000 Y5.000")));
        assert!(
            !output
                .lines()
                .any(|line| line.contains("X0.000 Y0.000 Z0.200"))
        );
    }

    #[test]
    fn geometry_outside_bed_is_rejected() {
        let (perimeters, solid, sparse) = sample_inputs();
        let config = GCodeConfig {
            bed_width: 5.0,
            ..Default::default()
        };
        assert!(matches!(
            generate_gcode(&perimeters, &solid, &sparse, &config),
            Err(GCodeError::CoordinateOutsideBed { .. })
        ));
    }

    #[test]
    fn optimization_preserves_extrusion_and_does_not_increase_travel() {
        let (perimeters, solid, sparse) = sample_inputs();
        let optimized =
            generate_gcode_with_report(&perimeters, &solid, &sparse, &GCodeConfig::default())
                .unwrap();
        let unoptimized_config = GCodeConfig {
            travel_optimization: false,
            ..Default::default()
        };
        let unoptimized =
            generate_gcode_with_report(&perimeters, &solid, &sparse, &unoptimized_config).unwrap();
        assert!(optimized.total_travel_distance <= unoptimized.total_travel_distance + 1e-9);
        let extrusion = |gcode: &str| {
            gcode
                .lines()
                .filter_map(|line| line.split(" E").nth(1))
                .filter_map(|value| value.split_whitespace().next())
                .filter_map(|value| value.parse::<f64>().ok())
                .filter(|value| *value > 0.0)
                .sum::<f64>()
        };
        assert!((extrusion(&optimized.gcode) - extrusion(&unoptimized.gcode)).abs() < 1e-9);
    }

    #[test]
    fn perimeter_vertex_contact_is_not_a_crossing() {
        assert!(!segments_intersect(
            (0.0, 0.0),
            (1.0, 1.0),
            (1.0, 1.0),
            (2.0, 1.0)
        ));
        assert!(segments_intersect(
            (0.0, 0.5),
            (2.0, 0.5),
            (1.0, 0.0),
            (1.0, 1.0)
        ));
    }

    #[test]
    fn aligned_seam_reuses_the_previous_layer_target() {
        let points = vec![
            Point2 { x: 0, y: 0 },
            Point2 { x: 10_000, y: 0 },
            Point2 {
                x: 10_000,
                y: 10_000,
            },
            Point2 { x: 0, y: 10_000 },
        ];
        let config = GCodeConfig {
            seam_position: SeamPosition::Aligned,
            ..Default::default()
        };
        let first = seam_points(&points, None, None, &config);
        let second = seam_points(&points, None, Some(first[0]), &config);
        assert_eq!(first[0], second[0]);
    }

    #[test]
    fn aligned_seams_are_tracked_per_wall_index() {
        let config = GCodeConfig {
            seam_position: SeamPosition::Aligned,
            ..Default::default()
        };
        let outer = vec![
            Point2 { x: 0, y: 0 },
            Point2 { x: 10_000, y: 0 },
            Point2 {
                x: 10_000,
                y: 10_000,
            },
            Point2 { x: 0, y: 10_000 },
        ];
        let inner = vec![
            Point2 { x: 2_000, y: 2_000 },
            Point2 { x: 8_000, y: 2_000 },
            Point2 { x: 8_000, y: 8_000 },
            Point2 { x: 2_000, y: 8_000 },
        ];
        let previous = HashMap::from([
            (
                0,
                Point2 {
                    x: 10_000,
                    y: 10_000,
                },
            ),
            (1, Point2 { x: 2_000, y: 8_000 }),
        ]);
        let outer_seam = seam_points(&outer, None, previous.get(&0).copied(), &config)[0];
        let inner_seam = seam_points(&inner, None, previous.get(&1).copied(), &config)[0];
        assert_eq!(
            outer_seam,
            Point2 {
                x: 10_000,
                y: 10_000
            }
        );
        assert_eq!(inner_seam, Point2 { x: 2_000, y: 8_000 });
    }
}
