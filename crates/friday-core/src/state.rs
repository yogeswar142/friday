use crate::coordinates::NormalizedPoint;
use crate::events::{ElementState, InputEvent, KeyCode, KeyboardEvent, MouseButton, MouseEvent};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Memory tracking last cursor positions per device
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CursorMemory {
    positions: HashMap<String, NormalizedPoint>,
}

impl CursorMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_last_position(&mut self, device_id: impl Into<String>, point: NormalizedPoint) {
        self.positions.insert(device_id.into(), point);
    }

    pub fn get_last_position(&self, device_id: &str) -> Option<NormalizedPoint> {
        self.positions.get(device_id).copied()
    }
}

/// Tracks currently held buttons and keys to guarantee safe releases upon disconnect
#[derive(Debug, Clone, Default)]
pub struct HeldInputState {
    pub held_mouse_buttons: HashSet<MouseButton>,
    pub held_keys: HashSet<KeyCode>,
    pub is_remotely_controlled: bool,
    pub emergency_escaped: bool,
}

impl HeldInputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.held_mouse_buttons.insert(button);
            }
            ElementState::Released => {
                self.held_mouse_buttons.remove(&button);
            }
        }
    }

    pub fn record_key(&mut self, key: KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.held_keys.insert(key);
            }
            ElementState::Released => {
                self.held_keys.remove(&key);
            }
        }
    }

    /// Emergency mechanism / Disconnect safety reset: Generate release events for all stuck inputs
    pub fn generate_safety_release_events(&mut self, timestamp: u32) -> Vec<InputEvent> {
        let mut release_events = Vec::new();

        for button in self.held_mouse_buttons.drain() {
            release_events.push(InputEvent::Mouse(MouseEvent::Button {
                button,
                state: ElementState::Released,
                timestamp,
            }));
        }

        for key in self.held_keys.drain() {
            release_events.push(InputEvent::Keyboard(KeyboardEvent {
                key,
                state: ElementState::Released,
                timestamp,
            }));
        }

        self.is_remotely_controlled = false;
        release_events
    }

    /// Trigger local emergency escape override
    pub fn trigger_emergency_escape(&mut self, timestamp: u32) -> Vec<InputEvent> {
        self.emergency_escaped = true;
        self.generate_safety_release_events(timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_held_input_state_safety_reset() {
        let mut state = HeldInputState::new();
        state.record_mouse_button(MouseButton::Left, ElementState::Pressed);
        state.record_mouse_button(MouseButton::Right, ElementState::Pressed);
        state.record_key(KeyCode::A, ElementState::Pressed);

        assert_eq!(state.held_mouse_buttons.len(), 2);
        assert_eq!(state.held_keys.len(), 1);

        let reset_events = state.generate_safety_release_events(100);
        assert_eq!(reset_events.len(), 3);
        assert_eq!(state.held_mouse_buttons.len(), 0);
        assert_eq!(state.held_keys.len(), 0);
    }
}
