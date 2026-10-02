use crate::traits::{CursorBackend, EventCallback, InputBackend, ScreenBackend};
use async_trait::async_trait;
use friday_core::{DisplayBounds, InputEvent, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct MockPlatformBackend {
    cursor_pos: Arc<Mutex<(i32, i32)>>,
    displays: Arc<Mutex<Vec<DisplayBounds>>>,
    injected_events: Arc<Mutex<Vec<InputEvent>>>,
    cursor_visible: Arc<Mutex<bool>>,
    capturing: Arc<Mutex<bool>>,
}

impl MockPlatformBackend {
    pub fn new(displays: Vec<DisplayBounds>) -> Self {
        Self {
            cursor_pos: Arc::new(Mutex::new((0, 0))),
            displays: Arc::new(Mutex::new(displays)),
            injected_events: Arc::new(Mutex::new(Vec::new())),
            cursor_visible: Arc::new(Mutex::new(true)),
            capturing: Arc::new(Mutex::new(false)),
        }
    }

    pub fn default_1080p() -> Self {
        Self::new(vec![DisplayBounds::new(0, 0, 1920, 1080, 1.0, true)])
    }

    pub async fn get_injected_events(&self) -> Vec<InputEvent> {
        self.injected_events.lock().await.clone()
    }

    pub async fn clear_injected_events(&self) {
        self.injected_events.lock().await.clear();
    }
}

#[async_trait]
impl InputBackend for MockPlatformBackend {
    async fn start_capture(&self, _callback: EventCallback) -> Result<()> {
        *self.capturing.lock().await = true;
        Ok(())
    }

    async fn stop_capture(&self) -> Result<()> {
        *self.capturing.lock().await = false;
        Ok(())
    }

    async fn inject_event(&self, event: &InputEvent) -> Result<()> {
        self.injected_events.lock().await.push(event.clone());
        Ok(())
    }
}

#[async_trait]
impl ScreenBackend for MockPlatformBackend {
    async fn get_displays(&self) -> Result<Vec<DisplayBounds>> {
        Ok(self.displays.lock().await.clone())
    }

    async fn get_primary_display(&self) -> Result<DisplayBounds> {
        let displays = self.displays.lock().await;
        displays
            .iter()
            .find(|d| d.is_primary)
            .cloned()
            .or_else(|| displays.first().cloned())
            .ok_or_else(|| friday_core::CoreError::RouterError("No displays found".into()))
    }
}

#[async_trait]
impl CursorBackend for MockPlatformBackend {
    async fn get_position(&self) -> Result<(i32, i32)> {
        Ok(*self.cursor_pos.lock().await)
    }

    async fn set_position(&self, x: i32, y: i32) -> Result<()> {
        *self.cursor_pos.lock().await = (x, y);
        Ok(())
    }

    async fn hide_cursor(&self) -> Result<()> {
        *self.cursor_visible.lock().await = false;
        Ok(())
    }

    async fn show_cursor(&self) -> Result<()> {
        *self.cursor_visible.lock().await = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use friday_core::{ElementState, MouseButton, MouseEvent};

    #[test]
    fn test_mock_platform_backend() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let backend = MockPlatformBackend::default_1080p();
            let display = backend.get_primary_display().await.unwrap();
            assert_eq!(display.width, 1920);

            backend.set_position(500, 300).await.unwrap();
            assert_eq!(backend.get_position().await.unwrap(), (500, 300));

            let evt = InputEvent::Mouse(MouseEvent::Button {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                timestamp: 10,
            });
            backend.inject_event(&evt).await.unwrap();
            let injected = backend.get_injected_events().await;
            assert_eq!(injected.len(), 1);
            assert_eq!(injected[0], evt);
        });
    }
}
