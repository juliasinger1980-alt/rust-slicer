use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::Duration,
};

use eframe::egui;
use slicer_core::{slice, Mesh, SeamPosition, SliceProgress, SliceResult, SliceSettings};

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct SettingField {
    path: &'static str,
    section: &'static str,
}

#[allow(dead_code)]
const SETTINGS_FIELDS: &[SettingField] = &[
    SettingField {
        path: "slice.layer_height",
        section: "Quality",
    },
    SettingField {
        path: "slice.first_layer_height",
        section: "Quality",
    },
    SettingField {
        path: "slice.scale",
        section: "Quality",
    },
    SettingField {
        path: "perimeter.wall_count",
        section: "Walls",
    },
    SettingField {
        path: "perimeter.line_width",
        section: "Walls",
    },
    SettingField {
        path: "perimeter.nozzle_diameter",
        section: "Printer/bed",
    },
    SettingField {
        path: "infill.density",
        section: "Infill",
    },
    SettingField {
        path: "infill.line_width",
        section: "Infill",
    },
    SettingField {
        path: "infill.base_angle",
        section: "Infill",
    },
    SettingField {
        path: "infill.wall_overlap",
        section: "Infill",
    },
    SettingField {
        path: "solid_infill.top_layers",
        section: "Infill",
    },
    SettingField {
        path: "solid_infill.bottom_layers",
        section: "Infill",
    },
    SettingField {
        path: "solid_infill.solid_line_width",
        section: "Infill",
    },
    SettingField {
        path: "gcode.seam_position",
        section: "Seams",
    },
    SettingField {
        path: "adhesion.skirt_loops",
        section: "Skirt/Brim",
    },
    SettingField {
        path: "adhesion.skirt_distance",
        section: "Skirt/Brim",
    },
    SettingField {
        path: "adhesion.brim_enabled",
        section: "Skirt/Brim",
    },
    SettingField {
        path: "adhesion.brim_width",
        section: "Skirt/Brim",
    },
    SettingField {
        path: "adhesion.brim_inner_holes",
        section: "Skirt/Brim",
    },
    SettingField {
        path: "cooling.enabled",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.small_perimeter_threshold_mm",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.small_perimeter_speed_mm_s",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.minimum_layer_time_s",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.minimum_print_speed_mm_s",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.fan_enabled",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.fan_off_first_layers",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.fan_full_speed_layer",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.fan_min_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "cooling.fan_max_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.nozzle_diameter",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.layer_height",
        section: "Quality",
    },
    SettingField {
        path: "gcode.first_layer_height",
        section: "Quality",
    },
    SettingField {
        path: "gcode.filament_diameter",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.filament_density_g_cm3",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.bed_width",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.bed_depth",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.bed_max_z",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.nozzle_temperature",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.first_layer_nozzle_temperature",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.bed_temperature",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.first_layer_bed_temperature",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.outer_wall_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.inner_wall_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.solid_infill_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.sparse_infill_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.travel_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.first_layer_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.first_layer_travel_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.fan_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.first_layer_fan_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.maximum_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.retraction_distance",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.retraction_speed",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.minimum_retraction_travel",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.travel_optimization",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.avoid_crossing_perimeters",
        section: "Cooling & speeds",
    },
    SettingField {
        path: "gcode.maximum_temperature",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.start_gcode",
        section: "Printer/bed",
    },
    SettingField {
        path: "gcode.end_gcode",
        section: "Printer/bed",
    },
];

struct WorkerMessage {
    progress: SliceProgress,
    result: Option<Result<SliceResult, String>>,
}

struct SlicerApp {
    settings: SliceSettings,
    mesh: Option<Mesh>,
    mesh_path: Option<PathBuf>,
    result: Option<SliceResult>,
    receiver: Option<Receiver<WorkerMessage>>,
    cancel: Option<Arc<AtomicBool>>,
    progress: SliceProgress,
    error: Option<String>,
    selected_layer: usize,
    zoom: f32,
    pan: egui::Vec2,
    visible: [bool; 9],
    show_all_layers: bool,
    last_folder: PathBuf,
    warning: Option<String>,
}

impl Default for SlicerApp {
    fn default() -> Self {
        Self {
            settings: SliceSettings::default(),
            mesh: None,
            mesh_path: None,
            result: None,
            receiver: None,
            cancel: None,
            progress: SliceProgress {
                phase: "idle",
                fraction: 0.0,
            },
            error: None,
            selected_layer: 0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            visible: [true; 9],
            show_all_layers: false,
            last_folder: PathBuf::new(),
            warning: None,
        }
    }
}

impl SlicerApp {
    fn open_stl(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.last_folder)
            .add_filter("STL", &["stl"])
            .pick_file()
        {
            self.last_folder = path.parent().unwrap_or(path.as_path()).to_path_buf();
            match Mesh::from_stl(&path) {
                Ok(mesh) => {
                    self.mesh = Some(mesh);
                    self.mesh_path = Some(path);
                    self.error = None;
                }

                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn settings_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_default()
            .join(".rust-slicer-ui-settings.toml")
    }

    fn folder_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_default()
            .join(".rust-slicer-ui-folder")
    }

    fn load_persistent_state(&mut self) {
        if let Ok(settings) = SliceSettings::load(Self::settings_path()) {
            self.settings = settings;
        }
        if let Ok(folder) = fs::read_to_string(Self::folder_path()) {
            self.last_folder = PathBuf::from(folder.trim());
        }
    }

    fn persist_state(&self) {
        let _ = self.settings.save(Self::settings_path());
        if !self.last_folder.as_os_str().is_empty() {
            let _ = fs::write(
                Self::folder_path(),
                self.last_folder.to_string_lossy().as_bytes(),
            );
        }
    }
    fn start_slice(&mut self) {
        let Some(mesh) = self.mesh.clone() else {
            self.error = Some("Open an STL before slicing.".into());
            return;
        };
        let settings = self.settings.clone();
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            let result = slice(
                &mesh,
                &settings,
                |progress| {
                    let _ = sender.send(WorkerMessage {
                        progress,
                        result: None,
                    });
                },
                &worker_cancel,
            )
            .map_err(|error| error.to_string());
            let _ = sender.send(WorkerMessage {
                progress: SliceProgress {
                    phase: "complete",
                    fraction: 1.0,
                },
                result: Some(result),
            });
        });
        self.receiver = Some(receiver);
        self.cancel = Some(cancel);
        self.result = None;
        self.error = None;
    }

    fn poll_worker(&mut self) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        let mut keep = true;
        while let Ok(message) = receiver.try_recv() {
            self.progress = message.progress;
            if let Some(result) = message.result {
                match result {
                    Ok(result) => self.result = Some(result),
                    Err(error) => self.error = Some(error),
                }
                self.cancel = None;
                keep = false;
            }
        }
        if keep {
            self.receiver = Some(receiver);
        }
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        self.section(ui, "Quality", |ui, settings| {
            drag(
                ui,
                "Layer height (mm)",
                &mut settings.slice.layer_height,
                0.05..=1.0,
            );
            drag(
                ui,
                "First layer height (mm)",
                &mut settings.slice.first_layer_height,
                0.05..=1.0,
            );
            drag(ui, "Scale", &mut settings.slice.scale, 1.0..=10_000.0);
        });
        self.section(ui, "Walls", |ui, settings| {
            ui.add(
                egui::Slider::new(&mut settings.perimeter.wall_count, 0..=20).text("Wall count"),
            );
            drag(
                ui,
                "Line width (mm)",
                &mut settings.perimeter.line_width,
                0.1..=2.0,
            );
            drag(
                ui,
                "Nozzle diameter (mm)",
                &mut settings.perimeter.nozzle_diameter,
                0.1..=2.0,
            );
        });
        self.section(ui, "Infill", |ui, settings| {
            drag(ui, "Density", &mut settings.infill.density, 0.01..=1.0);
            drag(
                ui,
                "Line width (mm)",
                &mut settings.infill.line_width,
                0.1..=2.0,
            );
            drag(
                ui,
                "Base angle (deg)",
                &mut settings.infill.base_angle,
                0.0..=180.0,
            );
            drag(
                ui,
                "Wall overlap (mm)",
                &mut settings.infill.wall_overlap,
                0.0..=0.5,
            );
            ui.add(
                egui::Slider::new(&mut settings.solid_infill.top_layers, 0..=20).text("Top layers"),
            );
            ui.add(
                egui::Slider::new(&mut settings.solid_infill.bottom_layers, 0..=20)
                    .text("Bottom layers"),
            );
            drag(
                ui,
                "Solid line width (mm)",
                &mut settings.solid_infill.solid_line_width,
                0.1..=2.0,
            );
        });
        self.section(ui, "Seams", |ui, settings| {
            egui::ComboBox::from_label("Seam position")
                .selected_text(format!("{:?}", settings.gcode.seam_position))
                .show_ui(ui, |ui| {
                    for position in [
                        SeamPosition::Nearest,
                        SeamPosition::Aligned,
                        SeamPosition::Rear,
                        SeamPosition::Random,
                    ] {
                        ui.selectable_value(
                            &mut settings.gcode.seam_position,
                            position,
                            format!("{position:?}"),
                        );
                    }
                });
        });
        self.section(ui, "Support", |ui, _| {
            ui.label("Supports will be available in a later milestone.");
        });
        self.section(ui, "Cooling & speeds", |ui, settings| {
            ui.checkbox(&mut settings.cooling.enabled, "Enable cooling");
            drag(
                ui,
                "Small perimeter threshold (mm)",
                &mut settings.cooling.small_perimeter_threshold_mm,
                0.0..=200.0,
            );
            drag(
                ui,
                "Small perimeter speed (mm/s)",
                &mut settings.cooling.small_perimeter_speed_mm_s,
                1.0..=300.0,
            );
            drag(
                ui,
                "Minimum layer time (s)",
                &mut settings.cooling.minimum_layer_time_s,
                0.0..=120.0,
            );
            drag(
                ui,
                "Minimum print speed (mm/s)",
                &mut settings.cooling.minimum_print_speed_mm_s,
                1.0..=100.0,
            );
            ui.checkbox(&mut settings.cooling.fan_enabled, "Enable fan");
            ui.add(
                egui::Slider::new(&mut settings.cooling.fan_off_first_layers, 0..=20)
                    .text("Fan-off layers"),
            );
            ui.add(
                egui::Slider::new(&mut settings.cooling.fan_full_speed_layer, 0..=100)
                    .text("Full fan layer"),
            );
            ui.add(
                egui::Slider::new(&mut settings.cooling.fan_min_speed, 0..=255).text("Fan minimum"),
            );
            ui.add(
                egui::Slider::new(&mut settings.cooling.fan_max_speed, 0..=255).text("Fan maximum"),
            );
            for (label, value) in [
                (
                    "Outer wall speed (mm/s)",
                    &mut settings.gcode.outer_wall_speed,
                ),
                (
                    "Inner wall speed (mm/s)",
                    &mut settings.gcode.inner_wall_speed,
                ),
                (
                    "Solid infill speed (mm/s)",
                    &mut settings.gcode.solid_infill_speed,
                ),
                (
                    "Sparse infill speed (mm/s)",
                    &mut settings.gcode.sparse_infill_speed,
                ),
                ("Travel speed (mm/s)", &mut settings.gcode.travel_speed),
                (
                    "First layer speed (mm/s)",
                    &mut settings.gcode.first_layer_speed,
                ),
                (
                    "First layer travel (mm/s)",
                    &mut settings.gcode.first_layer_travel_speed,
                ),
                ("Maximum speed (mm/s)", &mut settings.gcode.maximum_speed),
            ] {
                drag(ui, label, value, 1.0..=500.0);
            }
            ui.add(egui::Slider::new(&mut settings.gcode.fan_speed, 0..=255).text("Fan speed"));
            ui.add(
                egui::Slider::new(&mut settings.gcode.first_layer_fan_speed, 0..=255)
                    .text("First-layer fan speed"),
            );
            drag(
                ui,
                "Retraction distance (mm)",
                &mut settings.gcode.retraction_distance,
                0.0..=20.0,
            );
            drag(
                ui,
                "Retraction speed (mm/s)",
                &mut settings.gcode.retraction_speed,
                1.0..=100.0,
            );
            drag(
                ui,
                "Minimum retraction travel (mm)",
                &mut settings.gcode.minimum_retraction_travel,
                0.0..=100.0,
            );
            ui.checkbox(&mut settings.gcode.travel_optimization, "Optimize travel");
            ui.checkbox(
                &mut settings.gcode.avoid_crossing_perimeters,
                "Avoid crossing perimeters",
            );
        });
        self.section(ui, "Skirt/Brim", |ui, settings| {
            ui.add(
                egui::Slider::new(&mut settings.adhesion.skirt_loops, 0..=20).text("Skirt loops"),
            );
            drag(
                ui,
                "Skirt distance (mm)",
                &mut settings.adhesion.skirt_distance,
                0.0..=20.0,
            );
            ui.checkbox(&mut settings.adhesion.brim_enabled, "Enable brim");
            drag(
                ui,
                "Brim width (mm)",
                &mut settings.adhesion.brim_width,
                0.0..=30.0,
            );
            ui.checkbox(&mut settings.adhesion.brim_inner_holes, "Brim in holes");
        });
        self.section(ui, "Printer/bed", |ui, settings| {
            for (label, value) in [
                ("Nozzle diameter (mm)", &mut settings.gcode.nozzle_diameter),
                (
                    "Filament diameter (mm)",
                    &mut settings.gcode.filament_diameter,
                ),
                (
                    "Filament density (g/cm3)",
                    &mut settings.gcode.filament_density_g_cm3,
                ),
                ("Bed width (mm)", &mut settings.gcode.bed_width),
                ("Bed depth (mm)", &mut settings.gcode.bed_depth),
                ("Bed max Z (mm)", &mut settings.gcode.bed_max_z),
            ] {
                drag(ui, label, value, 0.1..=500.0);
            }
            ui.add(
                egui::Slider::new(&mut settings.gcode.nozzle_temperature, 0..=400)
                    .text("Nozzle temperature (C)"),
            );
            ui.add(
                egui::Slider::new(&mut settings.gcode.first_layer_nozzle_temperature, 0..=400)
                    .text("First nozzle (C)"),
            );
            ui.add(
                egui::Slider::new(&mut settings.gcode.bed_temperature, 0..=150)
                    .text("Bed temperature (C)"),
            );
            ui.add(
                egui::Slider::new(&mut settings.gcode.first_layer_bed_temperature, 0..=150)
                    .text("First bed (C)"),
            );
            ui.add(
                egui::Slider::new(&mut settings.gcode.maximum_temperature, 0..=400)
                    .text("Maximum temperature (C)"),
            );
            ui.collapsing("Start G-code", |ui| {
                ui.text_edit_multiline(&mut settings.gcode.start_gcode)
                    .on_hover_text("Commands emitted before the first layer");
            });
            ui.collapsing("End G-code", |ui| {
                ui.text_edit_multiline(&mut settings.gcode.end_gcode)
                    .on_hover_text("Commands emitted after the last layer");
            });
        });
    }

    fn section<F>(&mut self, ui: &mut egui::Ui, name: &str, body: F)
    where
        F: FnOnce(&mut egui::Ui, &mut SliceSettings),
    {
        egui::CollapsingHeader::new(name)
            .default_open(true)
            .show(ui, |ui| body(ui, &mut self.settings));
    }
}

impl eframe::App for SlicerApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_state();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        ctx.request_repaint_after(Duration::from_millis(50));
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open STL").clicked() {
                    self.open_stl();
                }
                if ui.button("Load settings").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.last_folder)
                        .add_filter("TOML", &["toml"])
                        .pick_file()
                    {
                        match SliceSettings::load(path) {
                            Ok(settings) => self.settings = settings,
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
                if ui.button("Save settings").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.last_folder)
                        .add_filter("TOML", &["toml"])
                        .save_file()
                    {
                        if let Err(error) = self.settings.save(path) {
                            self.error = Some(error.to_string());
                        }
                    }
                }
                if ui.button("Slice").clicked() && self.receiver.is_none() {
                    self.start_slice();
                }
                if ui.button("Export G-code").clicked() {
                    if let Some(result) = &self.result {
                        if !coordinates_in_bed(&result.gcode, &self.settings) {
                            self.error = Some("Export refused: G-code contains coordinates outside the bed.".into());
                        } else {
                            if self.settings.gcode.nozzle_temperature > self.settings.gcode.maximum_temperature
                                || self.settings.gcode.first_layer_nozzle_temperature > self.settings.gcode.maximum_temperature
                            {
                                self.warning = Some("Configured nozzle temperature exceeds the maximum and will be clamped.".into());
                            }
                        }
                        if self.error.is_none() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(&self.last_folder)
                            .add_filter("G-code", &["gcode"])
                            .save_file()
                        {
                            if let Err(error) = fs::write(path, &result.gcode) {
                                self.error = Some(error.to_string());
                            }
                        }
                        }
                    } else {
                        self.error = Some("Slice the model before exporting.".into());
                    }
                }
                if let Some(cancel) = &self.cancel {
                    if ui.button("Cancel").clicked() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
            });
        });
        egui::SidePanel::left("settings")
            .resizable(true)
            .show(ctx, |ui| self.settings_panel(ui));
        egui::SidePanel::right("stats")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Stats");
                ui.label(format!(
                    "Layers: {}",
                    self.result.as_ref().map_or(0, |r| r.layers.len())
                ));
                if let Some(result) = &self.result {
                    ui.label(format!(
                        "Time: {}",
                        format_duration(result.estimated_print_time_seconds)
                    ));
                    ui.label(format!(
                        "Filament: {:.2}m / {:.2}g",
                        result.filament_used_mm / 1000.0,
                        result.filament_used_grams
                    ));
                    ui.label(format!("Travel: {:.2}mm", result.total_travel_distance_mm));
                    ui.label(format!(
                        "Slice duration: {:.2}s",
                        result.slice_duration.as_secs_f64()
                    ));
                }
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::RED, error);
                }
                if let Some(warning) = &self.warning {
                    ui.colored_label(egui::Color32::YELLOW, warning);
                }
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.receiver.is_some() {
                ui.add(egui::ProgressBar::new(self.progress.fraction).text(self.progress.phase));
            }
            if let Some(result) = &self.result {
                if !result.layers.is_empty() {
                    self.selected_layer = self.selected_layer.min(result.layers.len() - 1);
                    ui.add(
                        egui::Slider::new(&mut self.selected_layer, 0..=result.layers.len() - 1)
                            .text("Layer"),
                    );
                    ui.checkbox(&mut self.show_all_layers, "Show all layers");
                    ui.horizontal_wrapped(|ui| {
                        for (visible, label) in self.visible.iter_mut().zip([
                            "Outer walls",
                            "Inner walls",
                            "Sparse infill",
                            "Solid infill",
                            "Bridges",
                            "Supports",
                            "Skirt",
                            "Brim",
                            "Seams",
                        ]) {
                            ui.checkbox(visible, label);
                        }
                    });
                    let (zoom, pan) = (&mut self.zoom, &mut self.pan);
                    draw_preview(
                        ui,
                        &result.layers,
                        self.selected_layer,
                        self.show_all_layers,
                        zoom,
                        pan,
                        &self.visible,
                        self.settings.gcode.bed_width,
                        self.settings.gcode.bed_depth,
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Slicing produced no layers for this model and settings.",
                    );
                }
            } else if let Some(mesh) = &self.mesh {
                ui.label("STL preview (slice the model to inspect its layers)");
                let (zoom, pan) = (&mut self.zoom, &mut self.pan);
                draw_mesh_preview(
                    ui,
                    mesh,
                    zoom,
                    pan,
                    self.settings.gcode.bed_width,
                    self.settings.gcode.bed_depth,
                );
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Open an STL to preview it, then slice to inspect its layers.")
                });
            }
        });
    }
}

fn drag(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>) {
    ui.add(egui::DragValue::new(value).speed(0.01).range(range))
        .on_hover_text(label);
}

fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total / 60) % 60,
        total % 60
    )
}

fn coordinates_in_bed(gcode: &str, settings: &SliceSettings) -> bool {
    gcode.lines().all(|line| {
        let mut x = None;
        let mut y = None;
        for token in line.split_whitespace() {
            if let Some(value) = token.strip_prefix('X').and_then(|v| v.parse::<f64>().ok()) {
                x = Some(value);
            }
            if let Some(value) = token.strip_prefix('Y').and_then(|v| v.parse::<f64>().ok()) {
                y = Some(value);
            }
        }
        x.is_none_or(|value| (0.0..=settings.gcode.bed_width).contains(&value))
            && y.is_none_or(|value| (0.0..=settings.gcode.bed_depth).contains(&value))
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_preview(
    ui: &mut egui::Ui,
    layers: &[slicer_core::SliceLayer],
    selected_layer: usize,
    show_all_layers: bool,
    zoom: &mut f32,
    pan: &mut egui::Vec2,
    visible: &[bool; 9],
    bed_width: f64,
    bed_depth: f64,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
    *pan += response.drag_delta();
    *zoom = (*zoom * (1.0 + response.ctx.input(|input| input.raw_scroll_delta.y) * 0.001))
        .clamp(0.1, 20.0);
    let scale = 2.0 * *zoom;
    let center = response.rect.center() + *pan
        - egui::vec2(
            bed_width as f32 * scale * 0.5,
            -bed_depth as f32 * scale * 0.5,
        );
    let bed = [
        slicer_core::Point2 { x: 0, y: 0 },
        slicer_core::Point2 {
            x: (bed_width * 1000.0) as i64,
            y: 0,
        },
        slicer_core::Point2 {
            x: (bed_width * 1000.0) as i64,
            y: (bed_depth * 1000.0) as i64,
        },
        slicer_core::Point2 {
            x: 0,
            y: (bed_depth * 1000.0) as i64,
        },
    ];
    let bed_polygon = slicer_core::Polygon {
        points: bed.to_vec(),
    };
    draw_polygon(
        &painter,
        &bed_polygon,
        center,
        scale,
        egui::Color32::DARK_GRAY,
    );
    let layers_to_draw: Vec<(usize, &slicer_core::SliceLayer)> = if show_all_layers {
        layers.iter().enumerate().collect()
    } else {
        vec![(selected_layer, &layers[selected_layer])]
    };
    for (layer_index, layer) in layers_to_draw {
        let selected = layer_index == selected_layer;
        draw_layer_geometry(&painter, layer, center, scale, visible, selected);
    }
}

fn draw_layer_geometry(
    painter: &egui::Painter,
    layer: &slicer_core::SliceLayer,
    center: egui::Pos2,
    scale: f32,
    visible: &[bool; 9],
    selected: bool,
) {
    let alpha = if selected { 255 } else { 48 };
    for wall in &layer.perimeter.walls {
        if visible[if wall.wall_index == 0 { 0 } else { 1 }] {
            let color = if wall.wall_index == 0 {
                egui::Color32::from_rgba_unmultiplied(50, 120, 220, alpha)
            } else {
                egui::Color32::from_rgba_unmultiplied(90, 180, 240, alpha)
            };
            draw_polygon(painter, &wall.polygon, center, scale, color);
        }
    }
    if visible[2] {
        for line in &layer.sparse_infill.lines {
            draw_line(
                painter,
                line.start,
                line.end,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(200, 200, 200, alpha),
            );
        }
    }
    if visible[3] {
        for line in &layer.solid_infill.solid_lines {
            draw_line(
                painter,
                line.start,
                line.end,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(255, 220, 0, alpha),
            );
        }
    }
    if visible[6] {
        for polygon in &layer.adhesion.skirts {
            draw_polygon(
                painter,
                polygon,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(150, 70, 180, alpha),
            );
        }
    }
    if visible[7] {
        for polygon in &layer.adhesion.brims {
            draw_polygon(
                painter,
                polygon,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(255, 165, 0, alpha),
            );
        }
    }
    if visible[4] {
        for polygon in &layer.bridges {
            draw_polygon(
                painter,
                polygon,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(255, 0, 0, alpha),
            );
        }
    }
    if visible[5] {
        for polygon in &layer.supports {
            draw_polygon(
                painter,
                polygon,
                center,
                scale,
                egui::Color32::from_rgba_unmultiplied(0, 200, 0, alpha),
            );
        }
    }
    if visible[8] {
        for point in &layer.seams {
            let position = center
                + egui::vec2(
                    point.x as f32 / 1000.0 * scale,
                    -point.y as f32 / 1000.0 * scale,
                );
            painter.circle_filled(
                position,
                3.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            );
        }
    }
}

fn draw_mesh_preview(
    ui: &mut egui::Ui,
    mesh: &Mesh,
    zoom: &mut f32,
    pan: &mut egui::Vec2,
    bed_width: f64,
    bed_depth: f64,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
    *pan += response.drag_delta();
    *zoom = (*zoom * (1.0 + response.ctx.input(|input| input.raw_scroll_delta.y) * 0.001))
        .clamp(0.1, 20.0);
    let scale = 2.0 * *zoom;
    let center = response.rect.center() + *pan
        - egui::vec2(
            bed_width as f32 * scale * 0.5,
            -bed_depth as f32 * scale * 0.5,
        );
    let bed_polygon = slicer_core::Polygon {
        points: vec![
            slicer_core::Point2 { x: 0, y: 0 },
            slicer_core::Point2 {
                x: (bed_width * 1000.0) as i64,
                y: 0,
            },
            slicer_core::Point2 {
                x: (bed_width * 1000.0) as i64,
                y: (bed_depth * 1000.0) as i64,
            },
            slicer_core::Point2 {
                x: 0,
                y: (bed_depth * 1000.0) as i64,
            },
        ],
    };
    draw_polygon(
        &painter,
        &bed_polygon,
        center,
        scale,
        egui::Color32::DARK_GRAY,
    );
    for triangle in &mesh.triangles {
        let points = triangle
            .vertices
            .iter()
            .map(|&index| {
                let vertex = mesh.vertices[index];
                center + egui::vec2(vertex.x as f32 * scale, -vertex.y as f32 * scale)
            })
            .collect::<Vec<_>>();
        if points.len() == 3 {
            painter.add(egui::Shape::closed_line(
                points,
                egui::Stroke::new(1.0_f32, egui::Color32::LIGHT_BLUE),
            ));
        }
    }
}

fn draw_polygon(
    painter: &egui::Painter,
    polygon: &slicer_core::Polygon,
    center: egui::Pos2,
    scale: f32,
    color: egui::Color32,
) {
    let points = polygon
        .points
        .iter()
        .map(|point| {
            center
                + egui::vec2(
                    point.x as f32 / 1000.0 * scale,
                    -point.y as f32 / 1000.0 * scale,
                )
        })
        .collect::<Vec<_>>();
    if points.len() > 1 {
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.0_f32, color)));
    }
}

fn draw_line(
    painter: &egui::Painter,
    start: slicer_core::Point2,
    end: slicer_core::Point2,
    center: egui::Pos2,
    scale: f32,
    color: egui::Color32,
) {
    let a = center
        + egui::vec2(
            start.x as f32 / 1000.0 * scale,
            -start.y as f32 / 1000.0 * scale,
        );
    let b = center
        + egui::vec2(
            end.x as f32 / 1000.0 * scale,
            -end.y as f32 / 1000.0 * scale,
        );
    painter.line_segment([a, b], egui::Stroke::new(1.0_f32, color));
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Rust Slicer",
        eframe::NativeOptions::default(),
        Box::new(|_| {
            let mut app = SlicerApp::default();
            app.load_persistent_state();
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_serialized_leaf_has_a_ui_field() {
        let value = toml::Value::try_from(SliceSettings::default()).unwrap();
        let mut leaves = Vec::new();
        fn walk(value: &toml::Value, prefix: String, leaves: &mut Vec<String>) {
            match value {
                toml::Value::Table(table) => {
                    for (key, value) in table {
                        walk(value, format!("{prefix}{key}."), leaves);
                    }
                }
                _ => leaves.push(prefix.trim_end_matches('.').to_string()),
            }
        }
        walk(&value, String::new(), &mut leaves);
        for leaf in leaves {
            assert!(
                SETTINGS_FIELDS.iter().any(|field| field.path == leaf),
                "missing UI field: {leaf}"
            );
        }
    }

    #[test]
    fn export_coordinate_check_rejects_points_outside_bed() {
        let settings = SliceSettings::default();
        assert!(coordinates_in_bed("G1 X10 Y20", &settings));
        assert!(!coordinates_in_bed("G1 X221 Y20", &settings));
        assert!(!coordinates_in_bed("G1 X10 Y221", &settings));
    }
}
