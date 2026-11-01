use std::path::Path;

use svg::node::element::{Group, Line, Path as SvgPath, Rectangle, Text};

use crate::{DEFAULT_SCALE, InfillLayer, PerimeterLayer, SliceError, SolidInfillLayer};

pub fn write_infill_layer_svg(
    layer: &PerimeterLayer,
    infill: &InfillLayer,
    path: impl AsRef<Path>,
) -> Result<(), SliceError> {
    let coordinates = layer
        .walls
        .iter()
        .flat_map(|wall| wall.polygon.points.iter())
        .chain(
            infill
                .lines
                .iter()
                .flat_map(|line| [&line.start, &line.end]),
        )
        .map(scaled_point)
        .collect::<Vec<_>>();
    let (min_x, max_x, min_y, max_y) = bounds(&coordinates);
    let mut group = transformed_group(min_y + max_y);
    for wall in &layer.walls {
        group = group.add(
            SvgPath::new()
                .set("d", polygon_path(&wall.polygon))
                .set("fill", "none")
                .set("stroke", "#377eb8")
                .set("stroke-width", "0.08")
                .set("vector-effect", "non-scaling-stroke"),
        );
    }
    for line in &infill.lines {
        group = group.add(line_element(line, "#e41a1c"));
    }
    write_document(group, min_x, max_x, min_y, max_y, path)
}

pub fn write_solid_infill_layer_svg(
    layer: &PerimeterLayer,
    infill: &SolidInfillLayer,
    path: impl AsRef<Path>,
) -> Result<(), SliceError> {
    let coordinates = layer
        .walls
        .iter()
        .flat_map(|wall| wall.polygon.points.iter())
        .chain(
            infill
                .sparse_region
                .iter()
                .chain(&infill.solid_region)
                .flat_map(|polygon| polygon.points.iter()),
        )
        .chain(
            infill
                .sparse_lines
                .iter()
                .flat_map(|line| [&line.start, &line.end]),
        )
        .chain(
            infill
                .solid_lines
                .iter()
                .flat_map(|line| [&line.start, &line.end]),
        )
        .map(scaled_point)
        .collect::<Vec<_>>();
    let (min_x, max_x, min_y, max_y) = bounds(&coordinates);
    let mut group = transformed_group(min_y + max_y);
    if !infill.sparse_region.is_empty() {
        group = group.add(
            SvgPath::new()
                .set("d", polygons_path(&infill.sparse_region))
                .set("fill", "#d3d3d3")
                .set("fill-rule", "evenodd")
                .set("stroke", "none"),
        );
    }
    if !infill.solid_region.is_empty() {
        group = group.add(
            SvgPath::new()
                .set("d", polygons_path(&infill.solid_region))
                .set("fill", "#ffd92f")
                .set("fill-rule", "evenodd")
                .set("stroke", "none"),
        );
    }
    for line in &infill.sparse_lines {
        group = group.add(line_element(line, "#377eb8"));
    }
    for line in &infill.solid_lines {
        group = group.add(line_element(line, "#4daf4a"));
    }
    write_document(group, min_x, max_x, min_y, max_y, path)
}

fn scaled_point(point: &crate::Point2) -> (f64, f64) {
    (
        point.x as f64 / DEFAULT_SCALE,
        point.y as f64 / DEFAULT_SCALE,
    )
}

fn bounds(coordinates: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    if coordinates.is_empty() {
        return (0.0, 1.0, 0.0, 1.0);
    }
    coordinates.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), &(x, y)| {
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    )
}

fn transformed_group(offset: f64) -> Group {
    Group::new().set("transform", format!("translate(0 {offset}) scale(1 -1)"))
}

fn polygon_path(polygon: &crate::Polygon) -> String {
    let mut data = String::new();
    for (index, point) in polygon.points.iter().enumerate() {
        data.push_str(if index == 0 { "M " } else { "L " });
        data.push_str(&format!(
            "{:.6},{:.6} ",
            scaled_point(point).0,
            scaled_point(point).1
        ));
    }
    data.push('Z');
    data
}

fn polygons_path(polygons: &[crate::Polygon]) -> String {
    polygons.iter().map(polygon_path).collect()
}

fn line_element(line: &crate::InfillLine, color: &str) -> Line {
    Line::new()
        .set("x1", line.start.x as f64 / DEFAULT_SCALE)
        .set("y1", line.start.y as f64 / DEFAULT_SCALE)
        .set("x2", line.end.x as f64 / DEFAULT_SCALE)
        .set("y2", line.end.y as f64 / DEFAULT_SCALE)
        .set("stroke", color)
        .set("stroke-width", "0.12")
        .set("vector-effect", "non-scaling-stroke")
}

fn write_document(
    group: Group,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    path: impl AsRef<Path>,
) -> Result<(), SliceError> {
    let margin = 1.0;
    let document = svg::Document::new()
        .set("width", "600")
        .set("height", "600")
        .set(
            "viewBox",
            format!(
                "{} {} {} {}",
                min_x - margin,
                min_y - margin,
                (max_x - min_x + 2.0 * margin).max(1.0),
                (max_y - min_y + 2.0 * margin).max(1.0)
            ),
        )
        .add(group)
        .add(legend());
    std::fs::write(path, document.to_string()).map_err(SliceError::Io)
}

fn legend() -> Group {
    let entries = [
        ("#377eb8", "Walls"),
        ("#d3d3d3", "Sparse infill"),
        ("#ffd92f", "Solid infill"),
    ];
    let mut group = Group::new().set("transform", "translate(12 20)");
    for (index, (color, label)) in entries.iter().enumerate() {
        let y = index as i32 * 18;
        group = group
            .add(
                Rectangle::new()
                    .set("x", 0)
                    .set("y", y)
                    .set("width", 12)
                    .set("height", 12)
                    .set("fill", *color)
                    .set("stroke", "black")
                    .set("stroke-width", 0.5),
            )
            .add(
                Text::new(*label)
                    .set("x", 18)
                    .set("y", y + 10)
                    .set("font-size", 10)
                    .set("font-family", "sans-serif"),
            );
    }
    group
}
