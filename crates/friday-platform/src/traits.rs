use async_trait::async_trait;
use friday_core::{DisplayBounds, InputEvent, Result};

pub type EventCallback = Box<dyn Fn(InputEvent) + Send + Sync + 'static>;

#[async_trait]
pub trait InputBackend: Send + Sync {
    async fn start_capture(&self, callback: EventCallback) -> Result<()>;
    async fn stop_capture(&self) -> Result<()>;
    async fn inject_event(&self, event: &InputEvent) -> Result<()>;
}

#[async_trait]
pub trait ScreenBackend: Send + Sync {
    async fn get_displays(&self) -> Result<Vec<DisplayBounds>>;
    async fn get_primary_display(&self) -> Result<DisplayBounds>;
}

#[async_trait]
pub trait CursorBackend: Send + Sync {
    async fn get_position(&self) -> Result<(i32, i32)>;
    async fn set_position(&self, x: i32, y: i32) -> Result<()>;
    async fn hide_cursor(&self) -> Result<()>;
    async fn show_cursor(&self) -> Result<()>;
}
