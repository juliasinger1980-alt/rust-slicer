use std::{env, fs, path::PathBuf};

use slicer_core::{
    Mesh, SliceSettings, generate_gcode_with_report, generate_infill, generate_perimeters,
    generate_solid_infills, slice_mesh, write_infill_layer_svg, write_layer_svg,
    write_perimeter_layer_svg, write_solid_infill_layer_svg,
};

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("error: {error}");
        eprintln!(
            "usage: slicer-cli <model.stl> [--svg-out <directory>] [--gcode-out <file.gcode>] [--config <settings.toml>] [--dump-default-config <settings.toml>]"
        );
        std::process::exit(1);
    }
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = None;
    let mut svg_output = None;
    let mut gcode_output = None;
    let mut settings_path = None;
    let mut dump_default_config = None;
    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--dump-default-config" => {
                dump_default_config = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("missing default config output path")?,
                ))
            }
            "--config" => {
                settings_path = Some(PathBuf::from(
                    arguments.next().ok_or("missing settings config path")?,
                ))
            }
            "--svg-out" => {
                svg_output = Some(PathBuf::from(
                    arguments.next().ok_or("missing SVG output directory")?,
                ))
            }
            "--gcode-out" => {
                gcode_output = Some(PathBuf::from(
                    arguments.next().ok_or("missing G-code output file")?,
                ))
            }
            option if option.starts_with('-') => {
                return Err(format!("unexpected option: {option}").into());
            }
            path => {
                if input.is_some() {
                    return Err("multiple input STL paths provided".into());
                }
                input = Some(PathBuf::from(path));
            }
        }
    }
    if let Some(path) = dump_default_config {
        SliceSettings::default().save(path)?;
        return Ok(());
    }
    let input = input.ok_or("missing input STL path")?;
    let settings = match settings_path {
        Some(path) => SliceSettings::load(path)?,
        None => SliceSettings::default(),
    };
    let had_svg_output = svg_output.is_some();

    let mesh = Mesh::from_stl(input)?;
    let layers = slice_mesh(&mesh, settings.slice)?;
    let perimeters = generate_perimeters(&layers, settings.perimeter)?;
    let infill = generate_infill(&perimeters, settings.infill)?;
    let solid_infill = generate_solid_infills(&perimeters, settings.infill, settings.solid_infill)?;

    if let Some(output) = svg_output {
        fs::create_dir_all(&output)?;
        for (index, ((layer, perimeter), infill_layer)) in
            layers.iter().zip(&perimeters).zip(&infill).enumerate()
        {
            write_layer_svg(layer, output.join(format!("layer_{index:04}.svg")))?;
            write_perimeter_layer_svg(perimeter, output.join(format!("perimeter_{index:04}.svg")))?;
            write_infill_layer_svg(
                perimeter,
                infill_layer,
                output.join(format!("infill_{index:04}.svg")),
            )?;
            write_solid_infill_layer_svg(
                perimeter,
                &solid_infill[index],
                output.join(format!("solid_infill_{index:04}.svg")),
            )?;
        }
    }
    if let Some(output) = gcode_output {
        let report =
            generate_gcode_with_report(&perimeters, &solid_infill, &infill, &settings.gcode)?;
        fs::write(output, report.gcode)?;
        println!("travel distance: {:.2} mm", report.total_travel_distance);
        println!(
            "estimated print time: {:.1} s",
            report.estimated_print_time_seconds
        );
    } else if !had_svg_output {
        return Err("provide --gcode-out or --svg-out".into());
    }

    Ok(())
}
