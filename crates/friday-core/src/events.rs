use serde::{Deserialize, Serialize};
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MouseButton {
    Left = 1,
    Right = 2,
    Middle = 3,
    Back = 4,
    Forward = 5,
    Other(u8) = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ElementState {
    Pressed = 1,
    Released = 2,
}

/// Ultra-compact mouse events optimized for high-frequency input transmission
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MouseEvent {
    /// Relative movement delta (dx, dy)
    MoveRel {
        dx: i16,
        dy: i16,
        timestamp: u32,
    },
    /// Normalized absolute movement coordinates (0..65535 -> 0.0..1.0)
    MoveAbs {
        x_norm: u16,
        y_norm: u16,
        timestamp: u32,
    },
    /// Button press or release
    Button {
        button: MouseButton,
        state: ElementState,
        timestamp: u32,
    },
    /// Mouse wheel / trackpad scroll
    Scroll {
        dx: i16,
        dy: i16,
        timestamp: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    Char(char),
    Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9, Key0,
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    LeftControl, RightControl,
    LeftShift, RightShift,
    LeftAlt, RightAlt,
    LeftSuper, RightSuper,
    Enter, Escape, Backspace, Tab, Space, CapsLock,
    Up, Down, Left, Right,
    Other(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardEvent {
    pub key: KeyCode,
    pub state: ElementState,
    pub timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
}

impl InputEvent {
    /// Fast binary serialization using bincode (or zero-copy binary layout)
    pub fn serialize_compact(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| CoreError::SerializationError(e.to_string()))
    }

    /// Fast binary deserialization
    pub fn deserialize_compact(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|e| CoreError::DeserializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_move_rel_serialization() {
        let event = InputEvent::Mouse(MouseEvent::MoveRel {
            dx: -12,
            dy: 45,
            timestamp: 123456,
        });
        let bytes = event.serialize_compact().unwrap();
        // Ensure compact size (< 20 bytes for movement)
        assert!(bytes.len() < 20);
        let decoded = InputEvent::deserialize_compact(&bytes).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_mouse_button_serialization() {
        let event = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Pressed,
            timestamp: 9999,
        });
        let bytes = event.serialize_compact().unwrap();
        let decoded = InputEvent::deserialize_compact(&bytes).unwrap();
        assert_eq!(event, decoded);
    }
}
