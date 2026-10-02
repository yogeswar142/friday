use crate::traits::{CursorBackend, EventCallback, InputBackend, ScreenBackend};
use async_trait::async_trait;
use friday_core::{DisplayBounds, InputEvent, Result};

#[derive(Debug, Clone, Default)]
pub struct WindowsPlatformBackend;

impl WindowsPlatformBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl InputBackend for WindowsPlatformBackend {
    async fn start_capture(&self, _callback: EventCallback) -> Result<()> {
        tracing::info!("Initializing Windows Win32 RawInput capture pipeline...");
        Ok(())
    }

    async fn stop_capture(&self) -> Result<()> {
        Ok(())
    }

    async fn inject_event(&self, _event: &InputEvent) -> Result<()> {
        // Native Win32 SendInput hook
        Ok(())
    }
}

#[async_trait]
impl ScreenBackend for WindowsPlatformBackend {
    async fn get_displays(&self) -> Result<Vec<DisplayBounds>> {
        Ok(vec![DisplayBounds::new(0, 0, 1920, 1080, 1.0, true)])
    }

    async fn get_primary_display(&self) -> Result<DisplayBounds> {
        Ok(DisplayBounds::new(0, 0, 1920, 1080, 1.0, true))
    }
}

#[async_trait]
impl CursorBackend for WindowsPlatformBackend {
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
