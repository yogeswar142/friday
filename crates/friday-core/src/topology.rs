use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::coordinates::{DisplayBounds, NormalizedPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub fn opposite(&self) -> Self {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenLayout {
    pub device_id: String,
    pub name: String,
    pub bounds: DisplayBounds,
}

/// Dynamic Topological Routing Map connecting N devices along screen edges
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenTopology {
    /// Maps (source_device_id, edge) -> target_device_id
    pub connections: HashMap<(String, Edge), String>,
    pub devices: HashMap<String, ScreenLayout>,
}

impl ScreenTopology {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_device(&mut self, layout: ScreenLayout) {
        self.devices.insert(layout.device_id.clone(), layout);
    }

    /// Link device A's edge to device B
    pub fn connect(&mut self, device_a: &str, edge: Edge, device_b: &str) {
        self.connections.insert((device_a.to_string(), edge), device_b.to_string());
        self.connections.insert((device_b.to_string(), edge.opposite()), device_a.to_string());
    }

    pub fn get_target_device(&self, source_device: &str, edge: Edge) -> Option<&String> {
        self.connections.get(&(source_device.to_string(), edge))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferEvent {
    pub source_device: String,
    pub target_device: String,
    pub edge: Edge,
    pub entry_point: NormalizedPoint,
}

/// Latency-critical Edge Detection and Input Router
#[derive(Debug)]
pub struct InputRouter {
    pub topology: ScreenTopology,
    pub current_device_id: String,
    pub edge_threshold_px: i32,
}

impl InputRouter {
    pub fn new(local_device_id: impl Into<String>, topology: ScreenTopology) -> Self {
        Self {
            topology,
            current_device_id: local_device_id.into(),
            edge_threshold_px: 2,
        }
    }

    /// Check if mouse coordinate (abs_x, abs_y) on current device triggers an edge transfer
    pub fn check_edge_transfer(&self, abs_x: i32, abs_y: i32) -> Option<TransferEvent> {
        let layout = self.topology.devices.get(&self.current_device_id)?;
        let bounds = &layout.bounds;

        let edge = if abs_x <= bounds.x + self.edge_threshold_px {
            Some(Edge::Left)
        } else if abs_x >= bounds.x + (bounds.width as i32) - 1 - self.edge_threshold_px {
            Some(Edge::Right)
        } else if abs_y <= bounds.y + self.edge_threshold_px {
            Some(Edge::Top)
        } else if abs_y >= bounds.y + (bounds.height as i32) - 1 - self.edge_threshold_px {
            Some(Edge::Bottom)
        } else {
            None
        };

        if let Some(triggered_edge) = edge {
            if let Some(target_id) = self.topology.get_target_device(&self.current_device_id, triggered_edge) {
                let norm = bounds.to_normalized(abs_x, abs_y).unwrap_or(NormalizedPoint { x: 0.5, y: 0.5 });
                
                // Calculate entry point on target screen
                let entry_point = match triggered_edge {
                    Edge::Right => NormalizedPoint { x: 0.0, y: norm.y },
                    Edge::Left => NormalizedPoint { x: 1.0, y: norm.y },
                    Edge::Top => NormalizedPoint { x: norm.x, y: 1.0 },
                    Edge::Bottom => NormalizedPoint { x: norm.x, y: 0.0 },
                };

                return Some(TransferEvent {
                    source_device: self.current_device_id.clone(),
                    target_device: target_id.clone(),
                    edge: triggered_edge,
                    entry_point,
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_edge_transfer() {
        let mut topology = ScreenTopology::new();
        let dev_a = ScreenLayout {
            device_id: "PC_A".to_string(),
            name: "PC A".to_string(),
            bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
        };
        let dev_b = ScreenLayout {
            device_id: "PC_B".to_string(),
            name: "PC B".to_string(),
            bounds: DisplayBounds::new(0, 0, 2560, 1440, 1.0, true),
        };

        topology.add_device(dev_a);
        topology.add_device(dev_b);
        topology.connect("PC_A", Edge::Right, "PC_B");

        let router = InputRouter::new("PC_A", topology);

        // Move mouse to right edge of PC_A (1919, 540)
        let transfer = router.check_edge_transfer(1919, 540).unwrap();
        assert_eq!(transfer.source_device, "PC_A");
        assert_eq!(transfer.target_device, "PC_B");
        assert_eq!(transfer.edge, Edge::Right);
        assert_eq!(transfer.entry_point.x, 0.0);
        assert!((transfer.entry_point.y - 0.5).abs() < 0.05);
    }
}
