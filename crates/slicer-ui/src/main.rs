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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    Stl,
    Sliced,
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
    show_3d_preview: bool,
    preview_mode: PreviewMode,
    rotation: egui::Vec2,
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
            show_3d_preview: true,
            preview_mode: PreviewMode::Stl,
            rotation: egui::vec2(0.65, 0.45),
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
        self.section(ui, "Printer settings", |ui, settings| {
            ui.strong("Bed");
            for (label, value) in [
                ("Bed width (mm)", &mut settings.gcode.bed_width),
                ("Bed depth (mm)", &mut settings.gcode.bed_depth),
                ("Bed max Z (mm)", &mut settings.gcode.bed_max_z),
            ] {
                drag(ui, label, value, 0.1..=500.0);
            }
            ui.separator();
            ui.strong("Nozzle and filament");
            drag(
                ui,
                "Nozzle diameter (mm)",
                &mut settings.gcode.nozzle_diameter,
                0.1..=2.0,
            );
            drag(
                ui,
                "Filament diameter (mm)",
                &mut settings.gcode.filament_diameter,
                1.0..=3.0,
            );
            drag(
                ui,
                "Filament density (g/cm3)",
                &mut settings.gcode.filament_density_g_cm3,
                0.5..=2.0,
            );
            ui.separator();
            ui.strong("Temperatures");
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
            ui.separator();
            ui.strong("Machine G-code");
            ui.collapsing("Start G-code", |ui| {
                ui.label("Start G-code commands");
                ui.text_edit_multiline(&mut settings.gcode.start_gcode)
                    .on_hover_text("Commands emitted before the first layer");
            });
            ui.collapsing("End G-code", |ui| {
                ui.label("End G-code commands");
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
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.settings_panel(ui));
            });
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
            ui.horizontal(|ui| {
                ui.label("Preview:");
                ui.selectable_value(&mut self.preview_mode, PreviewMode::Stl, "STL");
                ui.add_enabled_ui(self.result.is_some(), |ui| {
                    ui.selectable_value(&mut self.preview_mode, PreviewMode::Sliced, "Sliced");
                });
                ui.checkbox(&mut self.show_3d_preview, "3D model preview");
            });
            if self.preview_mode == PreviewMode::Sliced {
                if let Some(result) = &self.result {
                    if !result.layers.is_empty() {
                        self.selected_layer = self.selected_layer.min(result.layers.len() - 1);
                        ui.add(
                            egui::Slider::new(
                                &mut self.selected_layer,
                                0..=result.layers.len() - 1,
                            )
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
                    }
                }
            }
            if self.preview_mode == PreviewMode::Stl && self.show_3d_preview {
                if let Some(mesh) = &self.mesh {
                    let (zoom, pan, rotation) = (&mut self.zoom, &mut self.pan, &mut self.rotation);
                    draw_mesh_preview_3d(
                        ui,
                        mesh,
                        zoom,
                        pan,
                        rotation,
                        self.settings.gcode.bed_width,
                        self.settings.gcode.bed_depth,
                    );
                }
            } else if self.preview_mode == PreviewMode::Sliced && self.show_3d_preview {
                if let Some(result) = &self.result {
                    if result.layers.is_empty() {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "Slicing produced no layers for this model and settings.",
                        );
                    } else {
                        draw_sliced_preview_3d(
                            ui,
                            &result.layers,
                            self.selected_layer,
                            self.show_all_layers,
                            &self.visible,
                            &mut self.zoom,
                            &mut self.pan,
                            &mut self.rotation,
                            self.settings.slice.scale,
                        );
                    }
                }
            } else if self.preview_mode == PreviewMode::Sliced {
                if let Some(result) = &self.result {
                    if !result.layers.is_empty() {
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
                            self.settings.slice.scale,
                        );
                    } else {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "Slicing produced no layers for this model and settings.",
                        );
                    }
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
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.01).range(range))
            .on_hover_text(label);
    });
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

fn canvas_scale(rect: egui::Rect, bed_width: f64, bed_depth: f64, zoom: f32) -> f32 {
    let width = bed_width.max(f64::EPSILON) as f32;
    let depth = bed_depth.max(f64::EPSILON) as f32;
    let available_width = (rect.width() - 24.0).max(1.0);
    let available_height = (rect.height() - 24.0).max(1.0);
    (available_width / width)
        .min(available_height / depth)
        .mul_add(0.9, 0.0)
        * zoom
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
    coordinate_scale: f64,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
    *pan += response.drag_delta();
    *zoom = (*zoom * (1.0 + response.ctx.input(|input| input.raw_scroll_delta.y) * 0.001))
        .clamp(0.1, 20.0);
    let scale = canvas_scale(response.rect, bed_width, bed_depth, *zoom);
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
        1000.0,
        egui::Color32::DARK_GRAY,
    );
    let layers_to_draw: Vec<(usize, &slicer_core::SliceLayer)> = if show_all_layers {
        layers.iter().enumerate().collect()
    } else {
        vec![(selected_layer, &layers[selected_layer])]
    };
    for (layer_index, layer) in layers_to_draw {
        let selected = layer_index == selected_layer;
        draw_layer_geometry(
            &painter,
            layer,
            center,
            scale,
            coordinate_scale,
            visible,
            selected,
        );
    }
}

fn draw_layer_geometry(
    painter: &egui::Painter,
    layer: &slicer_core::SliceLayer,
    center: egui::Pos2,
    scale: f32,
    coordinate_scale: f64,
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
            draw_polygon(
                painter,
                &wall.polygon,
                center,
                scale,
                coordinate_scale,
                color,
            );
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
                coordinate_scale,
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
                coordinate_scale,
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
                coordinate_scale,
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
                coordinate_scale,
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
                coordinate_scale,
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
                coordinate_scale,
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
    let scale = canvas_scale(response.rect, bed_width, bed_depth, *zoom);
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
        1000.0,
        egui::Color32::DARK_GRAY,
    );
    let bounds = mesh_bounds(mesh);
    let model_offset = egui::vec2(
        bed_width as f32 * 0.5 - (bounds.0.x + bounds.1.x) * 0.5,
        bed_depth as f32 * 0.5 - (bounds.0.y + bounds.1.y) * 0.5,
    );
    for triangle in &mesh.triangles {
        let points = triangle
            .vertices
            .iter()
            .map(|&index| {
                let vertex = mesh.vertices[index];
                center
                    + egui::vec2(
                        (vertex.x as f32 + model_offset.x) * scale,
                        -(vertex.y as f32 + model_offset.y) * scale,
                    )
            })
            .collect::<Vec<_>>();
        if is_renderable_triangle(&points) {
            painter.add(egui::Shape::convex_polygon(
                points,
                egui::Color32::from_rgba_unmultiplied(80, 160, 230, 36),
                egui::Stroke::new(1.0_f32, egui::Color32::LIGHT_BLUE),
            ));
        }
    }
}

#[derive(Clone, Copy)]
struct ViewPoint {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy)]
struct PreviewTriangle {
    vertices: [ViewPoint; 3],
}

fn build_preview_triangles(mesh: &Mesh) -> Vec<PreviewTriangle> {
    mesh.triangles
        .iter()
        .filter_map(|triangle| {
            let [a, b, c] = triangle.vertices;
            let vertices = [
                mesh.vertices.get(a)?,
                mesh.vertices.get(b)?,
                mesh.vertices.get(c)?,
            ];
            Some(PreviewTriangle {
                vertices: [
                    ViewPoint {
                        x: vertices[0].x as f32,
                        y: vertices[0].y as f32,
                        z: vertices[0].z as f32,
                    },
                    ViewPoint {
                        x: vertices[1].x as f32,
                        y: vertices[1].y as f32,
                        z: vertices[1].z as f32,
                    },
                    ViewPoint {
                        x: vertices[2].x as f32,
                        y: vertices[2].y as f32,
                        z: vertices[2].z as f32,
                    },
                ],
            })
        })
        .collect()
}

fn draw_mesh_preview_3d(
    ui: &mut egui::Ui,
    mesh: &Mesh,
    zoom: &mut f32,
    pan: &mut egui::Vec2,
    rotation: &mut egui::Vec2,
    bed_width: f64,
    bed_depth: f64,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
    *rotation += response.drag_delta() * egui::vec2(0.01, 0.01);
    rotation.y = rotation.y.clamp(-1.45, 1.45);
    *zoom = (*zoom * (1.0 + response.ctx.input(|input| input.raw_scroll_delta.y) * 0.001))
        .clamp(0.1, 20.0);

    let bounds = mesh_bounds(mesh);
    let model_center = ViewPoint {
        x: (bounds.0.x + bounds.1.x) * 0.5,
        y: (bounds.0.y + bounds.1.y) * 0.5,
        z: bounds.0.z,
    };
    let camera_center = ViewPoint {
        x: bed_width as f32 * 0.5,
        y: bed_depth as f32 * 0.5,
        z: (bounds.1.z - bounds.0.z) * 0.5,
    };
    let extent = (bounds.1.x - bounds.0.x)
        .max(bounds.1.y - bounds.0.y)
        .max(bounds.1.z - bounds.0.z)
        .max(1.0);
    let available = response.rect.width().min(response.rect.height()).max(1.0);
    let scale = available / extent * 0.65 * *zoom;
    let origin = response.rect.center() + *pan;

    let project = |point: ViewPoint, model_space: bool| {
        let point = if model_space {
            ViewPoint {
                x: point.x - model_center.x + bed_width as f32 * 0.5,
                y: point.y - model_center.y + bed_depth as f32 * 0.5,
                z: point.z - model_center.z,
            }
        } else {
            point
        };
        let point = ViewPoint {
            x: point.x - camera_center.x,
            y: point.y - camera_center.y,
            z: point.z - camera_center.z,
        };
        let (sin_yaw, cos_yaw) = rotation.x.sin_cos();
        let x = point.x * cos_yaw - point.y * sin_yaw;
        let depth = point.x * sin_yaw + point.y * cos_yaw;
        let (sin_pitch, cos_pitch) = rotation.y.sin_cos();
        let height = depth * sin_pitch + point.z * cos_pitch;
        let depth = depth * cos_pitch - point.z * sin_pitch;
        (origin + egui::vec2(x * scale, -height * scale), depth)
    };

    let mut triangles = build_preview_triangles(mesh)
        .into_iter()
        .filter_map(|triangle| {
            if !preview_triangle_is_local(&triangle, bounds) {
                return None;
            }
            let camera_vertices = triangle.vertices.map(|vertex| {
                let point = ViewPoint {
                    x: vertex.x - model_center.x + bed_width as f32 * 0.5 - camera_center.x,
                    y: vertex.y - model_center.y + bed_depth as f32 * 0.5 - camera_center.y,
                    z: vertex.z - model_center.z - camera_center.z,
                };
                let (sin_yaw, cos_yaw) = rotation.x.sin_cos();
                let x = point.x * cos_yaw - point.y * sin_yaw;
                let depth = point.x * sin_yaw + point.y * cos_yaw;
                let (sin_pitch, cos_pitch) = rotation.y.sin_cos();
                [
                    x,
                    depth * sin_pitch + point.z * cos_pitch,
                    depth * cos_pitch - point.z * sin_pitch,
                ]
            });
            let normal = cross3(
                sub3(camera_vertices[1], camera_vertices[0]),
                sub3(camera_vertices[2], camera_vertices[0]),
            );
            let normal_length = length3(normal);
            if normal_length <= f32::EPSILON || normal[2].abs() / normal_length < 0.02 {
                return None;
            }
            let points = triangle.vertices.map(|vertex| project(vertex, true));
            let depth = points.iter().map(|(_, depth)| *depth).sum::<f32>() / 3.0;
            Some((points.to_vec(), depth))
        })
        .filter(|(points, _)| {
            let screen_points = points.iter().map(|(point, _)| *point).collect::<Vec<_>>();
            is_renderable_triangle(&screen_points)
                && projected_triangle_fits(&screen_points, response.rect)
        })
        .collect::<Vec<_>>();
    triangles.sort_by(|left, right| right.1.total_cmp(&left.1));

    for (points, _) in triangles {
        let points = points.into_iter().map(|(point, _)| point).collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            egui::Color32::from_rgba_unmultiplied(70, 155, 225, 220),
            egui::Stroke::NONE,
        ));
    }

    fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn length3(value: [f32; 3]) -> f32 {
        value
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt()
    }

    let bed_points = [
        ViewPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        ViewPoint {
            x: bed_width as f32,
            y: 0.0,
            z: 0.0,
        },
        ViewPoint {
            x: bed_width as f32,
            y: bed_depth as f32,
            z: 0.0,
        },
        ViewPoint {
            x: 0.0,
            y: bed_depth as f32,
            z: 0.0,
        },
    ]
    .map(|point| project(point, false).0);
    painter.add(egui::Shape::closed_line(
        bed_points.to_vec(),
        egui::Stroke::new(1.0_f32, egui::Color32::DARK_GRAY),
    ));
}

fn screen_triangle_area(points: &[egui::Pos2]) -> f32 {
    if points.len() != 3 {
        return 0.0;
    }
    ((points[0].x * (points[1].y - points[2].y)
        + points[1].x * (points[2].y - points[0].y)
        + points[2].x * (points[0].y - points[1].y))
        * 0.5)
        .abs()
}

fn is_renderable_triangle(points: &[egui::Pos2]) -> bool {
    if points.len() != 3
        || !points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    {
        return false;
    }
    let area = screen_triangle_area(points);
    let max_edge_squared = points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = points[(index + 1) % 3];
            point.distance_sq(next)
        })
        .fold(0.0_f32, f32::max);
    area > max_edge_squared * 0.0001
}

fn projected_triangle_fits(points: &[egui::Pos2], rect: egui::Rect) -> bool {
    if points.len() != 3 {
        return false;
    }
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let margin = 4.0;
    min_x >= rect.left() - margin
        && max_x <= rect.right() + margin
        && min_y >= rect.top() - margin
        && max_y <= rect.bottom() + margin
}

fn preview_triangle_is_local(triangle: &PreviewTriangle, bounds: (ViewPoint, ViewPoint)) -> bool {
    let extent = (bounds.1.x - bounds.0.x)
        .max(bounds.1.y - bounds.0.y)
        .max(bounds.1.z - bounds.0.z)
        .max(1.0);
    let mut edge_lengths_squared = triangle
        .vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            let next = triangle.vertices[(index + 1) % 3];
            let dx = vertex.x - next.x;
            let dy = vertex.y - next.y;
            let dz = vertex.z - next.z;
            dx * dx + dy * dy + dz * dz
        })
        .collect::<Vec<_>>();
    edge_lengths_squared.sort_by(f32::total_cmp);
    let largest = edge_lengths_squared[2];
    let a = triangle.vertices[0];
    let b = triangle.vertices[1];
    let c = triangle.vertices[2];
    let ab = [b.x - a.x, b.y - a.y, b.z - a.z];
    let ac = [c.x - a.x, c.y - a.y, c.z - a.z];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let doubled_area_squared = cross
        .iter()
        .map(|component| component * component)
        .sum::<f32>();
    !(largest > (extent * 0.75).powi(2) && doubled_area_squared < largest * largest * 0.01)
}

fn mesh_bounds(mesh: &Mesh) -> (ViewPoint, ViewPoint) {
    let mut min = ViewPoint {
        x: f32::INFINITY,
        y: f32::INFINITY,
        z: f32::INFINITY,
    };
    let mut max = ViewPoint {
        x: f32::NEG_INFINITY,
        y: f32::NEG_INFINITY,
        z: f32::NEG_INFINITY,
    };
    for vertex in &mesh.vertices {
        min.x = min.x.min(vertex.x as f32);
        min.y = min.y.min(vertex.y as f32);
        min.z = min.z.min(vertex.z as f32);
        max.x = max.x.max(vertex.x as f32);
        max.y = max.y.max(vertex.y as f32);
        max.z = max.z.max(vertex.z as f32);
    }
    (min, max)
}

#[allow(clippy::too_many_arguments)]
fn draw_sliced_preview_3d(
    ui: &mut egui::Ui,
    layers: &[slicer_core::SliceLayer],
    selected_layer: usize,
    show_all_layers: bool,
    visible: &[bool; 9],
    zoom: &mut f32,
    pan: &mut egui::Vec2,
    rotation: &mut egui::Vec2,
    coordinate_scale: f64,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
    *rotation += response.drag_delta() * egui::vec2(0.01, 0.01);
    rotation.y = rotation.y.clamp(-1.45, 1.45);
    *zoom = (*zoom * (1.0 + response.ctx.input(|input| input.raw_scroll_delta.y) * 0.001))
        .clamp(0.1, 20.0);

    let mut min = ViewPoint {
        x: f32::INFINITY,
        y: f32::INFINITY,
        z: f32::INFINITY,
    };
    let mut max = ViewPoint {
        x: f32::NEG_INFINITY,
        y: f32::NEG_INFINITY,
        z: f32::NEG_INFINITY,
    };
    for layer in layers {
        for wall in &layer.perimeter.walls {
            for point in &wall.polygon.points {
                min.x = min.x.min(point.x as f32 / coordinate_scale as f32);
                min.y = min.y.min(point.y as f32 / coordinate_scale as f32);
                max.x = max.x.max(point.x as f32 / coordinate_scale as f32);
                max.y = max.y.max(point.y as f32 / coordinate_scale as f32);
            }
        }
        min.z = min.z.min(layer.layer.z as f32);
        max.z = max.z.max(layer.layer.z as f32);
    }
    if !min.x.is_finite() {
        ui.label("The sliced result contains no drawable geometry.");
        return;
    }
    let model_center = ViewPoint {
        x: (min.x + max.x) * 0.5,
        y: (min.y + max.y) * 0.5,
        z: (min.z + max.z) * 0.5,
    };
    let extent = (max.x - min.x)
        .max(max.y - min.y)
        .max(max.z - min.z)
        .max(1.0);
    let scale = response.rect.width().min(response.rect.height()).max(1.0) / extent * 0.65 * *zoom;
    let origin = response.rect.center() + *pan;
    let project = |point: ViewPoint| {
        let point = ViewPoint {
            x: point.x - model_center.x,
            y: point.y - model_center.y,
            z: point.z - model_center.z,
        };
        let (sin_yaw, cos_yaw) = rotation.x.sin_cos();
        let x = point.x * cos_yaw - point.y * sin_yaw;
        let depth = point.x * sin_yaw + point.y * cos_yaw;
        let (sin_pitch, cos_pitch) = rotation.y.sin_cos();
        let height = depth * sin_pitch + point.z * cos_pitch;
        let depth = depth * cos_pitch - point.z * sin_pitch;
        (origin + egui::vec2(x * scale, -height * scale), depth)
    };

    let indices: Vec<usize> = if show_all_layers {
        (0..layers.len()).collect()
    } else {
        vec![selected_layer.min(layers.len() - 1)]
    };
    for index in indices {
        let layer = &layers[index];
        let alpha = if index == selected_layer { 255 } else { 48 };
        for wall in &layer.perimeter.walls {
            if visible[if wall.wall_index == 0 { 0 } else { 1 }] {
                let points = wall
                    .polygon
                    .points
                    .iter()
                    .map(|point| {
                        project(ViewPoint {
                            x: point.x as f32 / coordinate_scale as f32,
                            y: point.y as f32 / coordinate_scale as f32,
                            z: layer.layer.z as f32,
                        })
                        .0
                    })
                    .collect::<Vec<_>>();
                let color = if wall.wall_index == 0 {
                    egui::Color32::from_rgba_unmultiplied(50, 120, 220, alpha)
                } else {
                    egui::Color32::from_rgba_unmultiplied(90, 180, 240, alpha)
                };
                painter.add(egui::Shape::closed_line(
                    points,
                    egui::Stroke::new(1.0_f32, color),
                ));
            }
        }
        if visible[2] {
            for line in &layer.sparse_infill.lines {
                let color = egui::Color32::from_rgba_unmultiplied(200, 200, 200, alpha);
                let a = project(ViewPoint {
                    x: line.start.x as f32 / coordinate_scale as f32,
                    y: line.start.y as f32 / coordinate_scale as f32,
                    z: layer.layer.z as f32,
                })
                .0;
                let b = project(ViewPoint {
                    x: line.end.x as f32 / coordinate_scale as f32,
                    y: line.end.y as f32 / coordinate_scale as f32,
                    z: layer.layer.z as f32,
                })
                .0;
                painter.line_segment([a, b], egui::Stroke::new(1.0_f32, color));
            }
        }
        if visible[3] {
            for line in &layer.solid_infill.solid_lines {
                let color = if visible[3] {
                    egui::Color32::from_rgba_unmultiplied(255, 220, 0, alpha)
                } else {
                    egui::Color32::from_rgba_unmultiplied(200, 200, 200, alpha)
                };
                let a = project(ViewPoint {
                    x: line.start.x as f32 / coordinate_scale as f32,
                    y: line.start.y as f32 / coordinate_scale as f32,
                    z: layer.layer.z as f32,
                })
                .0;
                let b = project(ViewPoint {
                    x: line.end.x as f32 / coordinate_scale as f32,
                    y: line.end.y as f32 / coordinate_scale as f32,
                    z: layer.layer.z as f32,
                })
                .0;
                painter.line_segment([a, b], egui::Stroke::new(1.0_f32, color));
            }
        }
    }
}

fn draw_polygon(
    painter: &egui::Painter,
    polygon: &slicer_core::Polygon,
    center: egui::Pos2,
    scale: f32,
    coordinate_scale: f64,
    color: egui::Color32,
) {
    let points = polygon
        .points
        .iter()
        .map(|point| {
            center
                + egui::vec2(
                    point.x as f32 / coordinate_scale as f32 * scale,
                    -point.y as f32 / coordinate_scale as f32 * scale,
                )
        })
        .collect::<Vec<_>>();
    if points.len() > 1 {
        painter.add(egui::Shape::closed_line(
            points,
            egui::Stroke::new(1.0_f32, color),
        ));
    }
}

fn draw_line(
    painter: &egui::Painter,
    start: slicer_core::Point2,
    end: slicer_core::Point2,
    center: egui::Pos2,
    scale: f32,
    coordinate_scale: f64,
    color: egui::Color32,
) {
    let a = center
        + egui::vec2(
            start.x as f32 / coordinate_scale as f32 * scale,
            -start.y as f32 / coordinate_scale as f32 * scale,
        );
    let b = center
        + egui::vec2(
            end.x as f32 / coordinate_scale as f32 * scale,
            -end.y as f32 / coordinate_scale as f32 * scale,
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

    #[test]
    fn projected_triangle_filter_is_scale_independent() {
        let small = vec![
            egui::pos2(0.0, 0.0),
            egui::pos2(0.01, 0.0),
            egui::pos2(0.0, 0.01),
        ];
        let large = vec![
            egui::pos2(0.0, 0.0),
            egui::pos2(100.0, 0.0),
            egui::pos2(0.0, 100.0),
        ];
        assert!(is_renderable_triangle(&small));
        assert!(is_renderable_triangle(&large));
    }

    #[test]
    fn preview_triangle_filter_rejects_non_local_edges() {
        let bounds = (
            ViewPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            ViewPoint {
                x: 100.0,
                y: 100.0,
                z: 100.0,
            },
        );
        let triangle = PreviewTriangle {
            vertices: [
                ViewPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                ViewPoint {
                    x: 90.0,
                    y: 0.0,
                    z: 0.0,
                },
                ViewPoint {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
            ],
        };
        assert!(!preview_triangle_is_local(&triangle, bounds));
    }

    #[test]
    fn preview_triangles_reference_only_mesh_vertices() {
        let mesh = Mesh {
            vertices: vec![
                slicer_core::Vertex3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                slicer_core::Vertex3 {
                    x: 4.0,
                    y: 2.0,
                    z: 3.0,
                },
                slicer_core::Vertex3 {
                    x: 1.0,
                    y: 5.0,
                    z: 3.0,
                },
            ],
            triangles: vec![slicer_core::Triangle {
                vertices: [0, 1, 2],
            }],
        };
        let triangles = build_preview_triangles(&mesh);
        assert_eq!(triangles.len(), 1);
        let min = mesh_bounds(&mesh).0;
        let max = mesh_bounds(&mesh).1;
        for vertex in triangles[0].vertices {
            assert!((min.x..=max.x).contains(&vertex.x));
            assert!((min.y..=max.y).contains(&vertex.y));
            assert!((min.z..=max.z).contains(&vertex.z));
        }
    }
}
