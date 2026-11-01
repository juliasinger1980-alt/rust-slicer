use std::{env, fs, path::PathBuf};

use slicer_core::{
    GCodeConfig, InfillConfig, Mesh, PerimeterConfig, SliceConfig, SolidInfillConfig,
    generate_gcode, generate_infill, generate_perimeters, generate_solid_infills,
    load_gcode_config, slice_mesh, write_infill_layer_svg, write_layer_svg,
    write_perimeter_layer_svg, write_solid_infill_layer_svg,
};

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("error: {error}");
        eprintln!(
            "usage: slicer-cli <model.stl> [--svg-out <directory>] --gcode-out <file.gcode> [--gcode-config <file.toml>]"
        );
        std::process::exit(1);
    }
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let input = arguments.next().ok_or("missing input STL path")?;
    let mut svg_output = None;
    let mut gcode_output = None;
    let mut gcode_config = None;
    while let Some(option) = arguments.next() {
        match option.as_str() {
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
            "--gcode-config" => {
                gcode_config = Some(PathBuf::from(
                    arguments.next().ok_or("missing G-code config file")?,
                ))
            }
            _ => return Err(format!("unexpected option: {option}").into()),
        }
    }
    let had_svg_output = svg_output.is_some();

    let mesh = Mesh::from_stl(input)?;
    let config = SliceConfig::default();
    let layers = slice_mesh(&mesh, config)?;
    let perimeters = generate_perimeters(&layers, PerimeterConfig::default())?;
    let infill = generate_infill(&perimeters, InfillConfig::default())?;
    let solid_infill = generate_solid_infills(
        &perimeters,
        InfillConfig::default(),
        SolidInfillConfig::default(),
    )?;

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
        let config = match gcode_config {
            Some(path) => load_gcode_config(path)?,
            None => GCodeConfig::default(),
        };
        fs::write(
            output,
            generate_gcode(&perimeters, &solid_infill, &infill, &config)?,
        )?;
    } else if !had_svg_output {
        return Err("provide --gcode-out or --svg-out".into());
    }

    Ok(())
}
