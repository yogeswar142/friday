use serde::{Deserialize, Serialize};
use crate::{
    coordinates::NormalizedPoint,
    events::{InputEvent, MouseEvent},
    state::{CursorMemory, HeldInputState},
    topology::{CircularTopology, Edge, ScreenTopology, TransferEvent},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceOwnershipState {
    Active,
    Inactive,
    Transferring,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDevice {
    pub device_id: String,
    pub cursor_position: NormalizedPoint,
    pub topology: CircularTopology,
    pub transfer_state: DeviceOwnershipState,
}

/// Routing decision emitted for each incoming physical input event
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingDecision {
    /// Input belongs to local device — apply locally, do NOT send to network
    Local(InputEvent),
    /// Input belongs to remote device — send to network, do NOT apply locally
    Remote {
        target_device: String,
        event: InputEvent,
    },
    /// Ownership transferred between devices
    Transfer(TransferEvent),
    /// Event ignored / dropped (e.g. during transition)
    Drop,
}

/// Exclusive Cursor Ownership Engine
///
/// Enforces the invariant:
/// At any moment EXACTLY ONE device owns the physical input.
/// - Active device receives all inputs.
/// - Inactive devices receive ZERO input.
/// - No duplicate or mirrored inputs.
/// - Seamless Circular N-Device Mouse Routing across the entire ring of devices.
#[derive(Debug, Clone)]
pub struct ExclusiveOwnershipRouter {
    pub local_device_id: String,
    pub active_device_id: String,
    pub topology: CircularTopology,
    pub edge_threshold_px: i32,
    pub cursor_memory: CursorMemory,
    pub held_input: HeldInputState,
    pub remote_cursor_px: (f32, f32),
}

impl ExclusiveOwnershipRouter {
    pub fn new(local_device_id: impl Into<String>, topology: impl Into<CircularTopology>) -> Self {
        let local_id = local_device_id.into();
        let mut topo = topology.into();
        topo.set_active_device(local_id.clone());
        Self {
            active_device_id: local_id.clone(),
            local_device_id: local_id,
            topology: topo,
            edge_threshold_px: 3,
            cursor_memory: CursorMemory::new(),
            held_input: HeldInputState::new(),
            remote_cursor_px: (0.0, 0.0),
        }
    }

    pub fn from_screen_topology(local_device_id: impl Into<String>, topology: ScreenTopology) -> Self {
        Self::new(local_device_id, topology)
    }

    pub fn is_local_active(&self) -> bool {
        self.active_device_id == self.local_device_id
    }

    pub fn active_device_id(&self) -> &str {
        &self.active_device_id
    }

    /// Set edge threshold in pixels
    pub fn set_edge_threshold(&mut self, px: i32) {
        self.edge_threshold_px = px;
    }

    /// Process a physical mouse motion or button event.
    ///
    /// When Local is active:
    /// - Checks if cursor reached a screen boundary connecting to another device.
    /// - If boundary reached: transfers ownership, releases held buttons, and switches active device.
    /// - Otherwise: routes event locally.
    ///
    /// When Remote is active:
    /// - Translates relative delta against remote device screen geometry.
    /// - If remote cursor reaches a boundary connecting to next device:
    ///   transfers ownership to that device (whether local machine or another remote device in the circular ring).
    /// - Otherwise: routes event exclusively to active remote device.
    pub fn route_input(&mut self, event: InputEvent, current_local_pos: Option<(i32, i32)>) -> Vec<RoutingDecision> {
        let mut decisions = Vec::new();

        // Track held buttons for safety
        if let InputEvent::Mouse(MouseEvent::Button { button, state, .. }) = &event {
            self.held_input.record_mouse_button(*button, *state);
        }

        if self.is_local_active() {
            // ── LOCAL DEVICE IS ACTIVE ──
            if let Some((x, y)) = current_local_pos {
                if let Some(transfer) = self.check_local_edge_crossing(x, y) {
                    // 1. Release all held buttons on local device before handoff
                    let release_events = self.held_input.generate_safety_release_events(0);
                    for rel in release_events {
                        decisions.push(RoutingDecision::Local(rel));
                    }

                    // 2. Save local exit position
                    let local_layout = self.topology.devices.get(&self.local_device_id).cloned();
                    if let Some(layout) = &local_layout {
                        if let Ok(norm) = layout.bounds.to_normalized(x, y) {
                            self.cursor_memory.set_last_position(&self.local_device_id, norm);
                        }
                    }

                    // 3. Atomically transfer ownership to target
                    let target_id = transfer.target_device.clone();
                    self.active_device_id = target_id.clone();
                    self.topology.set_active_device(&target_id);

                    // 4. Initialize remote cursor position from entry point
                    if let Some(target_layout) = self.topology.devices.get(&target_id) {
                        let entry_x = transfer.entry_point.x * (target_layout.bounds.width as f32 - 1.0);
                        let entry_y = transfer.entry_point.y * (target_layout.bounds.height as f32 - 1.0);
                        self.remote_cursor_px = (entry_x, entry_y);
                    }

                    decisions.push(RoutingDecision::Transfer(transfer));
                    return decisions;
                }
            }

            // Normal local routing: affects ONLY local device
            decisions.push(RoutingDecision::Local(event));
        } else {
            // ── REMOTE DEVICE IS ACTIVE ──
            let remote_id = self.active_device_id.clone();
            let remote_layout = self.topology.devices.get(&remote_id).cloned();

            if let InputEvent::Mouse(MouseEvent::MoveRel { dx, dy, timestamp }) = &event {
                if let Some(layout) = remote_layout {
                    let w = layout.bounds.width as f32;
                    let h = layout.bounds.height as f32;

                    self.remote_cursor_px.0 += *dx as f32;
                    self.remote_cursor_px.1 += *dy as f32;

                    // Check if remote cursor reached a boundary connecting to next device in circular ring
                    let edge = if self.remote_cursor_px.0 <= 0.0 {
                        Some(Edge::Left)
                    } else if self.remote_cursor_px.0 >= w - 1.0 {
                        Some(Edge::Right)
                    } else if self.remote_cursor_px.1 <= 0.0 {
                        Some(Edge::Top)
                    } else if self.remote_cursor_px.1 >= h - 1.0 {
                        Some(Edge::Bottom)
                    } else {
                        None
                    };

                    if let Some(edge) = edge {
                        if let Some(target) = self.topology.neighbor_at_edge(&remote_id, edge) {
                            let target_id = target.to_string();

                            // 1. Release held buttons on active remote device before handoff
                            let release_events = self.held_input.generate_safety_release_events(*timestamp);
                            for rel in release_events {
                                decisions.push(RoutingDecision::Remote {
                                    target_device: remote_id.clone(),
                                    event: rel,
                                });
                            }

                            // 2. Save remote exit position
                            let norm_x = (self.remote_cursor_px.0 / w).clamp(0.0, 1.0);
                            let norm_y = (self.remote_cursor_px.1 / h).clamp(0.0, 1.0);
                            self.cursor_memory.set_last_position(&remote_id, NormalizedPoint { x: norm_x, y: norm_y });

                            // 3. Calculate entry point on target device
                            let entry_norm = CircularTopology::calculate_entry_point(edge, NormalizedPoint { x: norm_x, y: norm_y });

                            // 4. Transfer ownership to target_id (local machine or next remote device)
                            self.active_device_id = target_id.clone();
                            self.topology.set_active_device(&target_id);

                            // 5. If new target is a remote device, initialize its cursor coordinates
                            if target_id != self.local_device_id {
                                if let Some(target_layout) = self.topology.devices.get(&target_id) {
                                    let entry_x = entry_norm.x * (target_layout.bounds.width as f32 - 1.0);
                                    let entry_y = entry_norm.y * (target_layout.bounds.height as f32 - 1.0);
                                    self.remote_cursor_px = (entry_x, entry_y);
                                }
                            }

                            decisions.push(RoutingDecision::Transfer(TransferEvent {
                                source_device: remote_id,
                                target_device: target_id,
                                edge,
                                entry_point: entry_norm,
                            }));
                            return decisions;
                        }
                    }

                    // Clamp to remote display bounds
                    self.remote_cursor_px.0 = self.remote_cursor_px.0.clamp(0.0, w - 1.0);
                    self.remote_cursor_px.1 = self.remote_cursor_px.1.clamp(0.0, h - 1.0);
                }
            }

            // Normal remote routing: sent ONLY to remote active device
            decisions.push(RoutingDecision::Remote {
                target_device: remote_id,
                event,
            });
        }

        decisions
    }

    /// Check if local coordinate triggers an edge transfer to another device
    fn check_local_edge_crossing(&self, x: i32, y: i32) -> Option<TransferEvent> {
        let layout = self.topology.devices.get(&self.local_device_id)?;
        let bounds = &layout.bounds;

        let edge = if x <= bounds.x + self.edge_threshold_px {
            Some(Edge::Left)
        } else if x >= bounds.x + (bounds.width as i32) - 1 - self.edge_threshold_px {
            Some(Edge::Right)
        } else if y <= bounds.y + self.edge_threshold_px {
            Some(Edge::Top)
        } else if y >= bounds.y + (bounds.height as i32) - 1 - self.edge_threshold_px {
            Some(Edge::Bottom)
        } else {
            None
        }?;

        let target_device = self.topology.neighbor_at_edge(&self.local_device_id, edge)?.to_string();

        let norm_x = ((x - bounds.x) as f32 / bounds.width as f32).clamp(0.0, 1.0);
        let norm_y = ((y - bounds.y) as f32 / bounds.height as f32).clamp(0.0, 1.0);

        let entry_point = CircularTopology::calculate_entry_point(edge, NormalizedPoint { x: norm_x, y: norm_y });

        Some(TransferEvent {
            source_device: self.local_device_id.clone(),
            target_device,
            edge,
            entry_point,
        })
    }


    /// Safety reset on disconnect or emergency escape
    /// Restores ownership back to local machine and releases all held inputs
    pub fn force_restore_local_ownership(&mut self, timestamp: u32) -> Vec<RoutingDecision> {
        let mut decisions = Vec::new();
        let was_remote = !self.is_local_active();
        let previous_active = self.active_device_id.clone();

        // Release all held buttons
        let release_events = self.held_input.generate_safety_release_events(timestamp);

        if was_remote {
            // Send releases to remote device first to prevent stuck clicks
            for rel in &release_events {
                decisions.push(RoutingDecision::Remote {
                    target_device: previous_active.clone(),
                    event: rel.clone(),
                });
            }
        }

        // Send releases locally as well
        for rel in release_events {
            decisions.push(RoutingDecision::Local(rel));
        }

        // Restore local ownership atomically
        self.active_device_id = self.local_device_id.clone();
        self.topology.set_active_device(&self.local_device_id);

        if was_remote {
            let last_local = self.cursor_memory.get_last_position(&self.local_device_id)
                .unwrap_or(NormalizedPoint { x: 0.5, y: 0.5 });
            decisions.push(RoutingDecision::Transfer(TransferEvent {
                source_device: previous_active,
                target_device: self.local_device_id.clone(),
                edge: Edge::Left,
                entry_point: last_local,
            }));
        }

        decisions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplayBounds, ElementState, MouseButton, ScreenLayout};

    fn create_test_setup() -> ExclusiveOwnershipRouter {
        let mut topology = ScreenTopology::new();
        // G50: 1366x768
        topology.add_device(ScreenLayout {
            device_id: "G50".into(),
            name: "Lenovo G50".into(),
            bounds: DisplayBounds::new(0, 0, 1366, 768, 1.0, true),
        });
        // Yoga: 1280x800 placed to the right of G50
        topology.add_device(ScreenLayout {
            device_id: "Yoga".into(),
            name: "Lenovo Yoga".into(),
            bounds: DisplayBounds::new(1366, 0, 1280, 800, 1.0, false),
        });
        // Connect G50 Right <-> Yoga Left
        topology.connect("G50", Edge::Right, "Yoga");

        let mut router = ExclusiveOwnershipRouter::new("G50", topology);
        router.set_edge_threshold(3);
        router
    }

    #[test]
    fn test_1_g50_active_only_g50_receives_movement() {
        let mut router = create_test_setup();
        assert_eq!(router.active_device_id(), "G50");
        assert!(router.is_local_active());

        // Move inside G50 screen (e.g. at 500, 400)
        let move_evt = InputEvent::Mouse(MouseEvent::MoveRel { dx: 10, dy: -5, timestamp: 100 });
        let decisions = router.route_input(move_evt.clone(), Some((500, 400)));

        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0], RoutingDecision::Local(move_evt));
    }

    #[test]
    fn test_2_no_movement_sent_to_inactive_devices() {
        let mut router = create_test_setup();
        // While G50 is active, ZERO remote events are produced
        let move_evt = InputEvent::Mouse(MouseEvent::MoveRel { dx: 2, dy: 2, timestamp: 101 });
        let decisions = router.route_input(move_evt, Some((600, 300)));

        for d in &decisions {
            if let RoutingDecision::Remote { .. } = d {
                panic!("Inactive device Yoga must receive ZERO input while G50 is active!");
            }
        }
    }

    #[test]
    fn test_3_g50_to_yoga_edge_transfer() {
        let mut router = create_test_setup();
        // Cursor reaches G50 Right Edge: x = 1365 (1366 - 1)
        let move_evt = InputEvent::Mouse(MouseEvent::MoveRel { dx: 5, dy: 0, timestamp: 102 });
        let decisions = router.route_input(move_evt, Some((1365, 384)));

        assert_eq!(router.active_device_id(), "Yoga");
        assert!(!router.is_local_active());

        // Must emit a Transfer event to Yoga with entry at left edge (0.0)
        let transfer = decisions.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).expect("Expected Transfer event");

        assert_eq!(transfer.source_device, "G50");
        assert_eq!(transfer.target_device, "Yoga");
        assert_eq!(transfer.edge, Edge::Right);
        assert_eq!(transfer.entry_point.x, 0.0);
    }

    #[test]
    fn test_4_yoga_active_only_yoga_receives_movement() {
        let mut router = create_test_setup();
        // Force ownership to Yoga
        router.active_device_id = "Yoga".into();
        router.remote_cursor_px = (640.0, 400.0);

        let move_evt = InputEvent::Mouse(MouseEvent::MoveRel { dx: 15, dy: 10, timestamp: 103 });
        let decisions = router.route_input(move_evt.clone(), None);

        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0], RoutingDecision::Remote {
            target_device: "Yoga".into(),
            event: move_evt,
        });

        // Ensure NO local event produced
        for d in &decisions {
            if let RoutingDecision::Local(_) = d {
                panic!("G50 must receive ZERO input while Yoga is active!");
            }
        }
    }

    #[test]
    fn test_5_no_duplicate_local_and_remote_movement() {
        let mut router = create_test_setup();
        let move_evt = InputEvent::Mouse(MouseEvent::MoveRel { dx: 5, dy: 5, timestamp: 104 });

        // While G50 active
        let decisions = router.route_input(move_evt.clone(), Some((100, 100)));
        assert!(!decisions.iter().any(|d| matches!(d, RoutingDecision::Remote { .. })));

        // Switch to Yoga
        router.active_device_id = "Yoga".into();
        router.remote_cursor_px = (640.0, 400.0);

        let decisions = router.route_input(move_evt, None);
        assert!(!decisions.iter().any(|d| matches!(d, RoutingDecision::Local(_))));
    }

    #[test]
    fn test_6_yoga_to_g50_reverse_edge_transfer() {
        let mut router = create_test_setup();
        // Switch to Yoga near Left edge (x = 5.0)
        router.active_device_id = "Yoga".into();
        router.remote_cursor_px = (5.0, 400.0);

        // Move Left past edge (dx = -10) -> x becomes -5 <= 0.0 (Left edge)
        let move_left = InputEvent::Mouse(MouseEvent::MoveRel { dx: -10, dy: 0, timestamp: 105 });
        let decisions = router.route_input(move_left, None);

        // Ownership must return to G50
        assert_eq!(router.active_device_id(), "G50");
        assert!(router.is_local_active());

        let transfer = decisions.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).expect("Expected reverse Transfer event");

        assert_eq!(transfer.source_device, "Yoga");
        assert_eq!(transfer.target_device, "G50");
        assert_eq!(transfer.edge, Edge::Left);
    }

    #[test]
    fn test_7_cursor_position_preserved_for_inactive_devices() {
        let mut router = create_test_setup();
        // G50 exit at (1365, 500)
        let exit_y = 500;
        let _ = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 2, dy: 0, timestamp: 106 }),
            Some((1365, exit_y)),
        );

        // Check preserved position for G50
        let preserved = router.cursor_memory.get_last_position("G50").expect("Preserved pos");
        let expected_norm_y = exit_y as f32 / 768.0;
        assert!((preserved.y - expected_norm_y).abs() < 0.01);
    }

    #[test]
    fn test_8_held_buttons_safely_released_on_transfer() {
        let mut router = create_test_setup();

        // Press Left button on G50
        let btn_down = InputEvent::Mouse(MouseEvent::Button {
            button: MouseButton::Left,
            state: ElementState::Pressed,
            timestamp: 107,
        });
        let _ = router.route_input(btn_down, Some((500, 300)));
        assert!(router.held_input.held_mouse_buttons.contains(&MouseButton::Left));

        // Cross edge to Yoga
        let decisions = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 10, dy: 0, timestamp: 108 }),
            Some((1365, 300)),
        );

        // Must contain safety release event
        let released = decisions.iter().any(|d| match d {
            RoutingDecision::Local(InputEvent::Mouse(MouseEvent::Button { button, state, .. })) => {
                *button == MouseButton::Left && *state == ElementState::Released
            }
            _ => false,
        });
        assert!(released, "Held button must be released during ownership transfer");
        assert!(router.held_input.held_mouse_buttons.is_empty());
    }

    #[test]
    fn test_9_network_disconnect_safely_returns_local_control() {
        let mut router = create_test_setup();
        router.active_device_id = "Yoga".into();
        router.held_input.record_mouse_button(MouseButton::Right, ElementState::Pressed);

        // Simulate network disconnect / emergency escape
        let decisions = router.force_restore_local_ownership(999);

        assert_eq!(router.active_device_id(), "G50");
        assert!(router.is_local_active());
        assert!(router.held_input.held_mouse_buttons.is_empty());

        // Releases sent to remote Yoga AND local G50
        assert!(decisions.iter().any(|d| matches!(d, RoutingDecision::Remote { .. })));
        assert!(decisions.iter().any(|d| matches!(d, RoutingDecision::Local(_) )));
        assert!(decisions.iter().any(|d| matches!(d, RoutingDecision::Transfer(_) )));
    }

    #[test]
    fn test_10_circular_2_devices_infinite_right_loop() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "Laptop A".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "Laptop B".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // Pass 1: A Right -> B Left
        let dec1 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 10, dy: 0, timestamp: 1 }),
            Some((1919, 540)),
        );
        assert_eq!(router.active_device_id(), "B");
        let t1 = dec1.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t1.source_device, "A");
        assert_eq!(t1.target_device, "B");
        assert_eq!(t1.edge, Edge::Right);
        assert_eq!(t1.entry_point.x, 0.0); // Enters B at left edge

        // Pass 1: B Right -> A Left
        let dec2 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 2000, dy: 0, timestamp: 2 }),
            None,
        );
        assert_eq!(router.active_device_id(), "A");
        let t2 = dec2.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t2.source_device, "B");
        assert_eq!(t2.target_device, "A");
        assert_eq!(t2.edge, Edge::Right);
        assert_eq!(t2.entry_point.x, 0.0); // Enters A at left edge

        // Pass 2: A Right -> B Left again! (Circular continuation)
        let dec3 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 10, dy: 0, timestamp: 3 }),
            Some((1919, 540)),
        );
        assert_eq!(router.active_device_id(), "B");
        let t3 = dec3.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t3.source_device, "A");
        assert_eq!(t3.target_device, "B");
    }

    #[test]
    fn test_11_circular_2_devices_infinite_left_loop() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "Laptop A".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "Laptop B".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // Pass 1: A Left -> B Right
        let dec1 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: -10, dy: 0, timestamp: 1 }),
            Some((0, 540)),
        );
        assert_eq!(router.active_device_id(), "B");
        let t1 = dec1.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t1.source_device, "A");
        assert_eq!(t1.target_device, "B");
        assert_eq!(t1.edge, Edge::Left);
        assert_eq!(t1.entry_point.x, 1.0); // Enters B at right edge

        // Pass 1: B Left -> A Right
        let dec2 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: -2000, dy: 0, timestamp: 2 }),
            None,
        );
        assert_eq!(router.active_device_id(), "A");
        let t2 = dec2.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t2.source_device, "B");
        assert_eq!(t2.target_device, "A");
        assert_eq!(t2.edge, Edge::Left);
        assert_eq!(t2.entry_point.x, 1.0); // Enters A at right edge
    }

    #[test]
    fn test_12_circular_3_devices_right_circle_a_b_c_a() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "PC A (Local)".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "PC B (Remote 1)".into(),
                bounds: DisplayBounds::new(0, 0, 2560, 1440, 1.0, false),
            },
            ScreenLayout {
                device_id: "C".into(),
                name: "PC C (Remote 2)".into(),
                bounds: DisplayBounds::new(0, 0, 1366, 768, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // Step 1: A Right -> B Left
        let dec1 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 5, dy: 0, timestamp: 1 }),
            Some((1919, 540)),
        );
        assert_eq!(router.active_device_id(), "B");
        let t1 = dec1.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t1.source_device, "A");
        assert_eq!(t1.target_device, "B");
        assert_eq!(t1.entry_point.x, 0.0);

        // Step 2: B moves Right past edge (2560 width) -> C Left
        let dec2 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 3000, dy: 0, timestamp: 2 }),
            None,
        );
        assert_eq!(router.active_device_id(), "C");
        let t2 = dec2.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t2.source_device, "B");
        assert_eq!(t2.target_device, "C");
        assert_eq!(t2.edge, Edge::Right);
        assert_eq!(t2.entry_point.x, 0.0); // Enters C at left edge

        // Step 3: C moves Right past edge (1366 width) -> wraps back to A Left!
        let dec3 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 2000, dy: 0, timestamp: 3 }),
            None,
        );
        assert_eq!(router.active_device_id(), "A");
        let t3 = dec3.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t3.source_device, "C");
        assert_eq!(t3.target_device, "A");
        assert_eq!(t3.edge, Edge::Right);
        assert_eq!(t3.entry_point.x, 0.0); // Enters A at left edge
    }

    #[test]
    fn test_13_circular_3_devices_left_circle_a_c_b_a() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "PC A".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "PC B".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            },
            ScreenLayout {
                device_id: "C".into(),
                name: "PC C".into(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // Step 1: A Left -> C Right
        let dec1 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: -5, dy: 0, timestamp: 1 }),
            Some((0, 540)),
        );
        assert_eq!(router.active_device_id(), "C");
        let t1 = dec1.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t1.source_device, "A");
        assert_eq!(t1.target_device, "C");
        assert_eq!(t1.edge, Edge::Left);
        assert_eq!(t1.entry_point.x, 1.0); // Enters C at right edge

        // Step 2: C moves Left past edge -> B Right
        let dec2 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: -3000, dy: 0, timestamp: 2 }),
            None,
        );
        assert_eq!(router.active_device_id(), "B");
        let t2 = dec2.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t2.source_device, "C");
        assert_eq!(t2.target_device, "B");
        assert_eq!(t2.edge, Edge::Left);
        assert_eq!(t2.entry_point.x, 1.0); // Enters B at right edge

        // Step 3: B moves Left past edge -> wraps back to A Right!
        let dec3 = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: -3000, dy: 0, timestamp: 3 }),
            None,
        );
        assert_eq!(router.active_device_id(), "A");
        let t3 = dec3.iter().find_map(|d| match d {
            RoutingDecision::Transfer(t) => Some(t),
            _ => None,
        }).unwrap();
        assert_eq!(t3.source_device, "B");
        assert_eq!(t3.target_device, "A");
        assert_eq!(t3.edge, Edge::Left);
        assert_eq!(t3.entry_point.x, 1.0); // Enters A at right edge
    }

    #[test]
    fn test_14_held_button_safety_release_in_circular_transfer() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "A".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "B".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, false),
            },
            ScreenLayout {
                device_id: "C".into(),
                name: "C".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // Press Left button on A
        let _ = router.route_input(
            InputEvent::Mouse(MouseEvent::Button {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                timestamp: 10,
            }),
            Some((500, 500)),
        );
        assert!(router.held_input.held_mouse_buttons.contains(&MouseButton::Left));

        // Cross A -> B: safety release must be generated for A
        let dec = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 10, dy: 0, timestamp: 11 }),
            Some((999, 500)),
        );
        let rel = dec.iter().any(|d| matches!(d,
            RoutingDecision::Local(InputEvent::Mouse(MouseEvent::Button {
                button: MouseButton::Left,
                state: ElementState::Released,
                ..
            }))
        ));
        assert!(rel, "Held button on A must be released when transferring to B");
        assert!(router.held_input.held_mouse_buttons.is_empty());

        // Press Middle button on B
        let _ = router.route_input(
            InputEvent::Mouse(MouseEvent::Button {
                button: MouseButton::Middle,
                state: ElementState::Pressed,
                timestamp: 12,
            }),
            None,
        );
        assert!(router.held_input.held_mouse_buttons.contains(&MouseButton::Middle));

        // Cross B -> C: safety release must be sent to B
        let dec_bc = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 1500, dy: 0, timestamp: 13 }),
            None,
        );
        let rel_b = dec_bc.iter().any(|d| match d {
            RoutingDecision::Remote { target_device, event } => {
                target_device == "B" && matches!(event, InputEvent::Mouse(MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ElementState::Released,
                    ..
                }))
            }
            _ => false,
        });
        assert!(rel_b, "Held button on B must be released when transferring to C");
        assert!(router.held_input.held_mouse_buttons.is_empty());
    }

    #[test]
    fn test_15_inactive_devices_receive_zero_input_in_3_device_topology() {
        let ct = CircularTopology::from_ring(vec![
            ScreenLayout {
                device_id: "A".into(),
                name: "A".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, true),
            },
            ScreenLayout {
                device_id: "B".into(),
                name: "B".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, false),
            },
            ScreenLayout {
                device_id: "C".into(),
                name: "C".into(),
                bounds: DisplayBounds::new(0, 0, 1000, 1000, 1.0, false),
            },
        ]);
        let mut router = ExclusiveOwnershipRouter::new("A", ct);

        // When A is active: only Local decision, zero Remote to B or C
        let dec = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 1, dy: 1, timestamp: 1 }),
            Some((500, 500)),
        );
        assert_eq!(dec.len(), 1);
        assert!(matches!(dec[0], RoutingDecision::Local(_)));

        // Switch to B
        router.active_device_id = "B".into();
        router.remote_cursor_px = (500.0, 500.0);

        // When B is active: only Remote(B) decision, zero Local, zero Remote(C)
        let dec = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 1, dy: 1, timestamp: 2 }),
            None,
        );
        assert_eq!(dec.len(), 1);
        match &dec[0] {
            RoutingDecision::Remote { target_device, .. } => assert_eq!(target_device, "B"),
            _ => panic!("Expected remote decision for B"),
        }

        // Switch to C
        router.active_device_id = "C".into();
        router.remote_cursor_px = (500.0, 500.0);

        // When C is active: only Remote(C) decision, zero Local, zero Remote(B)
        let dec = router.route_input(
            InputEvent::Mouse(MouseEvent::MoveRel { dx: 1, dy: 1, timestamp: 3 }),
            None,
        );
        assert_eq!(dec.len(), 1);
        match &dec[0] {
            RoutingDecision::Remote { target_device, .. } => assert_eq!(target_device, "C"),
            _ => panic!("Expected remote decision for C"),
        }
    }
}
