use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::{
    AdhesionGeometry, InfillLayer, Layer, PerimeterLayer, SliceError, SliceSettings,
    SolidInfillLayer, generate_adhesion, generate_gcode_with_report_and_settings, generate_infill,
    generate_perimeters, generate_solid_infills,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliceProgress {
    pub phase: &'static str,
    pub fraction: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SliceLayer {
    pub layer: Layer,
    pub perimeter: PerimeterLayer,
    pub sparse_infill: InfillLayer,
    pub solid_infill: SolidInfillLayer,
    pub bridges: Vec<crate::Polygon>,
    pub supports: Vec<crate::Polygon>,
    pub adhesion: AdhesionGeometry,
    pub seams: Vec<crate::Point2>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SliceResult {
    pub layers: Vec<SliceLayer>,
    pub gcode: String,
    pub estimated_print_time_seconds: f64,
    pub filament_used_mm: f64,
    pub filament_used_grams: f64,
    pub total_travel_distance_mm: f64,
    pub slice_duration: Duration,
}

pub fn slice(
    mesh: &crate::Mesh,
    settings: &SliceSettings,
    progress: impl Fn(SliceProgress) + Send + Sync,
    cancel: &AtomicBool,
) -> Result<SliceResult, SliceError> {
    let started = Instant::now();
    let check = |phase: &'static str, fraction: f32| -> Result<(), SliceError> {
        progress(SliceProgress { phase, fraction });
        if cancel.load(Ordering::Relaxed) {
            Err(SliceError::Cancelled)
        } else {
            Ok(())
        }
    };

    check("slicing", 0.05)?;
    let layers = crate::slice_mesh(mesh, settings.slice)?;
    check("perimeters", 0.25)?;
    let perimeters = generate_perimeters(&layers, settings.perimeter)?;
    check("infill", 0.45)?;
    let sparse = generate_infill(&perimeters, settings.infill)?;
    let solid = generate_solid_infills(&perimeters, settings.infill, settings.solid_infill)?;
    check("adhesion", 0.65)?;
    let first_adhesion = layers
        .first()
        .map(|layer| generate_adhesion(layer, settings.adhesion, settings.gcode.nozzle_diameter))
        .transpose()?
        .unwrap_or_default();
    check("g-code", 0.8)?;
    let report = generate_gcode_with_report_and_settings(
        &perimeters,
        &solid,
        &sparse,
        Some(&layers),
        &settings.adhesion,
        &settings.cooling,
        &settings.gcode,
    )
    .map_err(|error| SliceError::GCode(error.to_string()))?;
    check("complete", 1.0)?;

    let filament_used_mm = report.total_extrusion_mm;
    let filament_area = std::f64::consts::PI * settings.gcode.filament_diameter.powi(2) / 4.0;
    let filament_used_grams =
        filament_used_mm * filament_area / 1000.0 * settings.gcode.filament_density_g_cm3;
    let result_layers = layers
        .into_iter()
        .zip(perimeters)
        .zip(sparse)
        .zip(solid)
        .enumerate()
        .map(
            |(index, (((layer, perimeter), sparse_infill), solid_infill))| SliceLayer {
                layer,
                perimeter,
                sparse_infill,
                solid_infill,
                adhesion: if index == 0 {
                    first_adhesion.clone()
                } else {
                    AdhesionGeometry::default()
                },
                seams: Vec::new(),
                bridges: Vec::new(),
                supports: Vec::new(),
            },
        )
        .collect();
    Ok(SliceResult {
        layers: result_layers,
        gcode: report.gcode,
        estimated_print_time_seconds: report.estimated_print_time_seconds,
        filament_used_mm,
        filament_used_grams,
        total_travel_distance_mm: report.total_travel_distance,
        slice_duration: started.elapsed(),
    })
}

pub fn shared_cancel_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_stops_before_processing_mesh() {
        let cancel = AtomicBool::new(true);
        let mesh = crate::Mesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        };
        let result = slice(&mesh, &SliceSettings::default(), |_| {}, &cancel);
        assert!(matches!(result, Err(SliceError::Cancelled)));
    }
}
