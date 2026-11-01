use std::{fmt, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    AdhesionConfig, CoolingConfig, GCodeConfig, InfillConfig, PerimeterConfig, SliceConfig,
    SolidInfillConfig,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SliceSettings {
    #[serde(default)]
    pub slice: SliceConfig,
    #[serde(default)]
    pub perimeter: PerimeterConfig,
    #[serde(default)]
    pub infill: InfillConfig,
    #[serde(default)]
    pub solid_infill: SolidInfillConfig,
    #[serde(default)]
    pub adhesion: AdhesionConfig,
    #[serde(default)]
    pub cooling: CoolingConfig,
    #[serde(default)]
    pub gcode: GCodeConfig,
}

impl PartialEq for SliceSettings {
    fn eq(&self, other: &Self) -> bool {
        match (toml::to_string(self), toml::to_string(other)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }
}

impl SliceSettings {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let text = fs::read_to_string(path).map_err(SettingsError::Io)?;
        toml::from_str(&text).map_err(|error| SettingsError::Toml(error.to_string()))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SettingsError> {
        let text =
            toml::to_string_pretty(self).map_err(|error| SettingsError::Toml(error.to_string()))?;
        fs::write(path, text).map_err(SettingsError::Io)
    }
}

#[derive(Debug)]
pub enum SettingsError {
    Io(std::io::Error),
    Toml(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "settings I/O failed: {error}"),
            Self::Toml(error) => write!(formatter, "invalid settings TOML: {error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_preserves_values() {
        let original = SliceSettings {
            adhesion: crate::AdhesionConfig {
                skirt_loops: 2,
                skirt_distance: 4.0,
                brim_enabled: true,
                brim_width: 1.2,
                brim_inner_holes: true,
            },
            cooling: crate::CoolingConfig {
                enabled: true,
                minimum_layer_time_s: 8.0,
                fan_enabled: true,
                fan_off_first_layers: 2,
                fan_full_speed_layer: 5,
                fan_min_speed: 20,
                fan_max_speed: 200,
                ..Default::default()
            },
            gcode: crate::GCodeConfig {
                seam_position: crate::SeamPosition::Aligned,
                travel_speed: 123.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let path =
            std::env::temp_dir().join(format!("rust-slicer-settings-{}.toml", std::process::id()));
        original.save(&path).unwrap();
        let decoded = SliceSettings::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(decoded, original);
    }
}
