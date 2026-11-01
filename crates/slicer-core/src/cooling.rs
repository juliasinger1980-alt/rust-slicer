use serde::{Deserialize, Serialize};

use crate::SliceError;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CoolingConfig {
    pub enabled: bool,
    pub small_perimeter_threshold_mm: f64,
    pub small_perimeter_speed_mm_s: f64,
    pub minimum_layer_time_s: f64,
    pub minimum_print_speed_mm_s: f64,
    pub fan_enabled: bool,
    pub fan_off_first_layers: usize,
    pub fan_full_speed_layer: usize,
    pub fan_min_speed: u8,
    pub fan_max_speed: u8,
}

impl Default for CoolingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            small_perimeter_threshold_mm: 0.0,
            small_perimeter_speed_mm_s: 20.0,
            minimum_layer_time_s: 0.0,
            minimum_print_speed_mm_s: 10.0,
            fan_enabled: false,
            fan_off_first_layers: 1,
            fan_full_speed_layer: 0,
            fan_min_speed: 0,
            fan_max_speed: 255,
        }
    }
}

impl CoolingConfig {
    pub(crate) fn validate(self) -> Result<Self, SliceError> {
        if !self.small_perimeter_threshold_mm.is_finite()
            || !self.small_perimeter_speed_mm_s.is_finite()
            || !self.minimum_layer_time_s.is_finite()
            || !self.minimum_print_speed_mm_s.is_finite()
            || self.small_perimeter_threshold_mm < 0.0
            || self.small_perimeter_speed_mm_s <= 0.0
            || self.minimum_layer_time_s < 0.0
            || self.minimum_print_speed_mm_s <= 0.0
            || self.fan_min_speed > self.fan_max_speed
        {
            return Err(SliceError::InvalidCoolingConfiguration);
        }
        Ok(self)
    }
}
