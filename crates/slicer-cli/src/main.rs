use std::{env, fs, path::PathBuf};

use slicer_core::{Mesh, SliceConfig, slice_mesh, write_layer_svg};

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("error: {error}");
        eprintln!("usage: slicer-cli <model.stl> --svg-out <directory>");
        std::process::exit(1);
    }
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let input = arguments.next().ok_or("missing input STL path")?;
    let option = arguments.next().ok_or("missing --svg-out option")?;
    if option != "--svg-out" {
        return Err(format!("expected --svg-out, got {option}").into());
    }
    let output = arguments.next().ok_or("missing SVG output directory")?;
    if arguments.next().is_some() {
        return Err("unexpected extra argument".into());
    }

    let output = PathBuf::from(output);
    fs::create_dir_all(&output)?;
    let mesh = Mesh::from_stl(input)?;
    let config = SliceConfig::default();
    let layers = slice_mesh(&mesh, config)?;

    println!("layers: {}", layers.len());
    println!("layer height: {:.3} mm", config.layer_height);
    for (index, layer) in layers.iter().enumerate() {
        let path = output.join(format!("layer_{index:04}.svg"));
        write_layer_svg(layer, &path)?;
    }

    Ok(())
}
