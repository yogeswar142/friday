use crate::traits::{CursorBackend, EventCallback, InputBackend, ScreenBackend};
use async_trait::async_trait;
use friday_core::{CoreError, DisplayBounds, InputEvent, Result};

#[derive(Debug, Clone)]
pub struct LinuxPlatformBackend {
    pub is_wayland: bool,
}

impl LinuxPlatformBackend {
    pub fn new() -> Self {
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|v| v == "wayland")
                .unwrap_or(false);
        Self { is_wayland }
    }
}

impl Default for LinuxPlatformBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputBackend for LinuxPlatformBackend {
    async fn start_capture(&self, _callback: EventCallback) -> Result<()> {
        if self.is_wayland {
            tracing::info!("Initializing Linux Wayland (libei/portal) input capture pipeline...");
        } else {
            tracing::info!("Initializing Linux X11 (XTest/XRecord) input capture pipeline...");
        }
        Ok(())
    }

    async fn stop_capture(&self) -> Result<()> {
        Ok(())
    }

    async fn inject_event(&self, _event: &InputEvent) -> Result<()> {
        // Native injection implementation hook
        Ok(())
    }
}

#[async_trait]
impl ScreenBackend for LinuxPlatformBackend {
    async fn get_displays(&self) -> Result<Vec<DisplayBounds>> {
        // Fallback display query
        Ok(vec![DisplayBounds::new(0, 0, 1920, 1080, 1.0, true)])
    }

    async fn get_primary_display(&self) -> Result<DisplayBounds> {
        let displays = self.get_displays().await?;
        displays
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::RouterError("No display found".into()))
    }
}

#[async_trait]
impl CursorBackend for LinuxPlatformBackend {
    async fn get_position(&self) -> Result<(i32, i32)> {
        Ok((960, 540))
    }

    async fn set_position(&self, _x: i32, _y: i32) -> Result<()> {
        Ok(())
    }

    async fn hide_cursor(&self) -> Result<()> {
        Ok(())
    }

    async fn show_cursor(&self) -> Result<()> {
        Ok(())
    }
}
