use std::path::Path;

use svg::node::element::{Group, Line, Path as SvgPath};

use crate::{DEFAULT_SCALE, InfillLayer, PerimeterLayer, SliceError};

pub fn write_infill_layer_svg(
    layer: &PerimeterLayer,
    infill: &InfillLayer,
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
        .chain(infill.lines.iter().flat_map(|line| {
            [
                (
                    line.start.x as f64 / DEFAULT_SCALE,
                    line.start.y as f64 / DEFAULT_SCALE,
                ),
                (
                    line.end.x as f64 / DEFAULT_SCALE,
                    line.end.y as f64 / DEFAULT_SCALE,
                ),
            ]
        }))
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
    let mut group = Group::new().set(
        "transform",
        format!("translate(0 {}) scale(1 -1)", min_y + max_y),
    );
    for wall in &layer.walls {
        let mut data = String::new();
        for (index, point) in wall.polygon.points.iter().enumerate() {
            data.push_str(if index == 0 { "M " } else { "L " });
            data.push_str(&format!(
                "{:.6},{:.6} ",
                point.x as f64 / DEFAULT_SCALE,
                point.y as f64 / DEFAULT_SCALE
            ));
        }
        data.push('Z');
        group = group.add(
            SvgPath::new()
                .set("d", data)
                .set("fill", "none")
                .set("stroke", "#377eb8")
                .set("stroke-width", "0.08")
                .set("vector-effect", "non-scaling-stroke"),
        );
    }
    for line in &infill.lines {
        group = group.add(
            Line::new()
                .set("x1", line.start.x as f64 / DEFAULT_SCALE)
                .set("y1", line.start.y as f64 / DEFAULT_SCALE)
                .set("x2", line.end.x as f64 / DEFAULT_SCALE)
                .set("y2", line.end.y as f64 / DEFAULT_SCALE)
                .set("stroke", "#e41a1c")
                .set("stroke-width", "0.12")
                .set("vector-effect", "non-scaling-stroke"),
        );
    }
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
        .add(group);
    std::fs::write(path, document.to_string()).map_err(SliceError::Io)
}
