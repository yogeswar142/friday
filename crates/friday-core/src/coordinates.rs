use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

/// Represents physical/pixel dimensions of a display
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub is_primary: bool,
}

impl DisplayBounds {
    pub fn new(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        scale_factor: f32,
        is_primary: bool,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            scale_factor: if scale_factor <= 0.0 {
                1.0
            } else {
                scale_factor
            },
            is_primary,
        }
    }

    /// Convert pixel coordinate (abs_x, abs_y) within display to normalized coordinates [0.0, 1.0]
    pub fn to_normalized(&self, abs_x: i32, abs_y: i32) -> Result<NormalizedPoint> {
        let rel_x = (abs_x - self.x) as f32;
        let rel_y = (abs_y - self.y) as f32;

        if self.width == 0 || self.height == 0 {
            return Err(CoreError::RouterError(
                "Display bounds width or height is zero".into(),
            ));
        }

        let norm_x = (rel_x / self.width as f32).clamp(0.0, 1.0);
        let norm_y = (rel_y / self.height as f32).clamp(0.0, 1.0);

        Ok(NormalizedPoint {
            x: norm_x,
            y: norm_y,
        })
    }

    /// Convert normalized point [0.0, 1.0] to physical pixel coordinate on this display
    pub fn to_pixel(&self, norm: NormalizedPoint) -> (i32, i32) {
        let px = self.x + (norm.x * (self.width.saturating_sub(1)) as f32).round() as i32;
        let py = self.y + (norm.y * (self.height.saturating_sub(1)) as f32).round() as i32;
        (px, py)
    }
}

/// Normalized screen coordinate [0.0, 1.0]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPoint {
    pub x: f32,
    pub y: f32,
}

impl NormalizedPoint {
    pub fn new(x: f32, y: f32) -> Result<Self> {
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(CoreError::InvalidCoordinates(x, y));
        }
        Ok(Self { x, y })
    }

    pub fn to_u16(&self) -> (u16, u16) {
        let u_x = (self.x * 65535.0).round() as u16;
        let u_y = (self.y * 65535.0).round() as u16;
        (u_x, u_y)
    }

    pub fn from_u16(u_x: u16, u_y: u16) -> Self {
        Self {
            x: u_x as f32 / 65535.0,
            y: u_y as f32 / 65535.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_normalization() {
        let display = DisplayBounds::new(0, 0, 1920, 1080, 1.0, true);
        let norm = display.to_normalized(960, 540).unwrap();
        assert!((norm.x - 0.5).abs() < 0.01);
        assert!((norm.y - 0.5).abs() < 0.01);

        let (px, py) = display.to_pixel(norm);
        assert_eq!((px, py), (960, 540));
    }

    #[test]
    fn test_u16_conversion() {
        let norm = NormalizedPoint::new(0.75, 0.25).unwrap();
        let (ux, uy) = norm.to_u16();
        let norm2 = NormalizedPoint::from_u16(ux, uy);
        assert!((norm.x - norm2.x).abs() < 0.001);
        assert!((norm.y - norm2.y).abs() < 0.001);
    }
}
