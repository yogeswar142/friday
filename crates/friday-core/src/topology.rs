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

    pub fn to_circular(&self) -> CircularTopology {
        self.clone().into()
    }
}

impl From<ScreenTopology> for CircularTopology {
    fn from(st: ScreenTopology) -> Self {
        let mut ring: Vec<String> = Vec::new();
        // Follow Right connections to establish the natural order of the ring
        let mut start_candidates: Vec<String> = Vec::new();
        for (src, edge) in st.connections.keys() {
            if *edge == Edge::Right && !start_candidates.contains(src) {
                start_candidates.push(src.clone());
            }
        }
        let start = start_candidates.first().or_else(|| st.devices.keys().next()).cloned();
        if let Some(first) = start {
            let mut curr = first;
            let mut visited = std::collections::HashSet::new();
            while !visited.contains(&curr) && st.devices.contains_key(&curr) {
                visited.insert(curr.clone());
                ring.push(curr.clone());
                if let Some(next) = st.connections.get(&(curr.clone(), Edge::Right)) {
                    curr = next.clone();
                } else {
                    break;
                }
            }
        }
        for id in st.devices.keys() {
            if !ring.contains(id) {
                ring.push(id.clone());
            }
        }
        let active_device = ring.first().cloned().unwrap_or_default();
        Self {
            ring,
            devices: st.devices,
            active_device,
            custom_links: st.connections,
        }
    }
}


/// Circular N-Device Mouse Routing Topology
///
/// Models connected computers as an ordered circular ring:
/// `[D_0, D_1, ..., D_{N-1}]`
///
/// In this circular topology:
/// - Moving RIGHT: `D_i` RIGHT edge -> `D_{(i+1)%N}` LEFT edge
///   `D_0 -> D_1 -> ... -> D_{N-1} -> D_0 -> ...`
/// - Moving LEFT: `D_i` LEFT edge -> `D_{(i-1+N)%N}` RIGHT edge
///   `D_0 -> D_{N-1} -> ... -> D_1 -> D_0 -> ...`
///
/// For N = 2:
/// - RIGHT: A.Right -> B.Left, B.Right -> A.Left
/// - LEFT: A.Left -> B.Right, B.Left -> A.Right
///
/// For N = 3:
/// - RIGHT: A.Right -> B.Left, B.Right -> C.Left, C.Right -> A.Left
/// - LEFT: A.Left -> C.Right, C.Left -> B.Right, B.Left -> A.Right
///
/// Cursor Continuity:
/// Perpendicular coordinate (y for horizontal, x for vertical) is preserved
/// proportionally across any resolution differences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircularTopology {
    /// Ordered circular ring of device IDs
    pub ring: Vec<String>,
    /// Device metadata and screen boundaries
    pub devices: HashMap<String, ScreenLayout>,
    /// The single authoritative active device ID
    pub active_device: String,
    /// Optional directional overrides (e.g. for vertical links)
    pub custom_links: HashMap<(String, Edge), String>,
}

impl CircularTopology {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct circular topology from an ordered list of device layouts
    pub fn from_ring(layouts: Vec<ScreenLayout>) -> Self {
        let mut topology = Self::new();
        for layout in layouts {
            topology.add_device(layout);
        }
        topology
    }

    /// Construct circular topology from existing device map and an ordered ring
    pub fn from_devices_and_ring(devices: HashMap<String, ScreenLayout>, ring: Vec<String>) -> Self {
        let active_device = ring.first().cloned().unwrap_or_default();
        Self {
            ring,
            devices,
            active_device,
            custom_links: HashMap::new(),
        }
    }

    /// Add a device to the topology ring
    pub fn add_device(&mut self, layout: ScreenLayout) {
        let id = layout.device_id.clone();
        if !self.ring.contains(&id) {
            self.ring.push(id.clone());
        }
        if self.active_device.is_empty() {
            self.active_device = id.clone();
        }
        self.devices.insert(id, layout);
    }

    /// Remove a device from the topology
    pub fn remove_device(&mut self, device_id: &str) -> Option<ScreenLayout> {
        self.ring.retain(|id| id != device_id);
        if self.active_device == device_id {
            self.active_device = self.ring.first().cloned().unwrap_or_default();
        }
        self.devices.remove(device_id)
    }

    /// Set the circular ring order explicitly
    pub fn set_ring(&mut self, ring: Vec<String>) {
        if !ring.is_empty() && (self.active_device.is_empty() || !ring.contains(&self.active_device)) {
            self.active_device = ring[0].clone();
        }
        self.ring = ring;
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    pub fn devices(&self) -> &HashMap<String, ScreenLayout> {
        &self.devices
    }

    pub fn ring(&self) -> &[String] {
        &self.ring
    }

    pub fn get_device(&self, device_id: &str) -> Option<&ScreenLayout> {
        self.devices.get(device_id)
    }

    pub fn active_device(&self) -> &str {
        &self.active_device
    }

    pub fn set_active_device(&mut self, device_id: impl Into<String>) {
        self.active_device = device_id.into();
    }

    /// Get right neighbor in circular ring:
    /// D_i -> D_{(i + 1) % N}
    pub fn right_neighbor(&self, device_id: &str) -> Option<&str> {
        if let Some(custom) = self.custom_links.get(&(device_id.to_string(), Edge::Right)) {
            return Some(custom.as_str());
        }
        if self.ring.len() < 2 {
            return None;
        }
        let pos = self.ring.iter().position(|id| id == device_id)?;
        let next_idx = (pos + 1) % self.ring.len();
        Some(self.ring[next_idx].as_str())
    }

    /// Get left neighbor in circular ring:
    /// D_i -> D_{(i + N - 1) % N}
    pub fn left_neighbor(&self, device_id: &str) -> Option<&str> {
        if let Some(custom) = self.custom_links.get(&(device_id.to_string(), Edge::Left)) {
            return Some(custom.as_str());
        }
        if self.ring.len() < 2 {
            return None;
        }
        let pos = self.ring.iter().position(|id| id == device_id)?;
        let prev_idx = (pos + self.ring.len() - 1) % self.ring.len();
        Some(self.ring[prev_idx].as_str())
    }

    /// Get neighbor for any edge
    pub fn neighbor_at_edge(&self, device_id: &str, edge: Edge) -> Option<&str> {
        if let Some(custom) = self.custom_links.get(&(device_id.to_string(), edge)) {
            return Some(custom.as_str());
        }
        match edge {
            Edge::Right => self.right_neighbor(device_id),
            Edge::Left => self.left_neighbor(device_id),
            Edge::Top | Edge::Bottom => None,
        }
    }

    /// Add a custom directional link (e.g. for Top/Bottom edges)
    pub fn set_custom_link(&mut self, source_id: impl Into<String>, edge: Edge, target_id: impl Into<String>) {
        self.custom_links.insert((source_id.into(), edge), target_id.into());
    }

    /// Calculate the opposite corresponding entry point preserving perpendicular coordinate
    pub fn calculate_entry_point(edge: Edge, exit_point: NormalizedPoint) -> NormalizedPoint {
        match edge {
            Edge::Right => NormalizedPoint { x: 0.0, y: exit_point.y.clamp(0.0, 1.0) },
            Edge::Left => NormalizedPoint { x: 1.0, y: exit_point.y.clamp(0.0, 1.0) },
            Edge::Top => NormalizedPoint { x: exit_point.x.clamp(0.0, 1.0), y: 1.0 },
            Edge::Bottom => NormalizedPoint { x: exit_point.x.clamp(0.0, 1.0), y: 0.0 },
        }
    }

    /// Atomically transfer ownership from source_id crossing `edge`
    pub fn transfer(&mut self, source_id: &str, edge: Edge, exit_point: NormalizedPoint) -> Option<TransferEvent> {
        let target_id = self.neighbor_at_edge(source_id, edge)?.to_string();
        let entry_point = Self::calculate_entry_point(edge, exit_point);
        self.active_device = target_id.clone();
        Some(TransferEvent {
            source_device: source_id.to_string(),
            target_device: target_id,
            edge,
            entry_point,
        })
    }

    /// Convert to legacy ScreenTopology if needed
    pub fn to_screen_topology(&self) -> ScreenTopology {
        let mut st = ScreenTopology::new();
        st.devices = self.devices.clone();
        if self.ring.len() == 2 {
            st.connect(&self.ring[0], Edge::Right, &self.ring[1]);
        } else if self.ring.len() > 2 {
            for i in 0..self.ring.len() {
                let curr = &self.ring[i];
                let next = &self.ring[(i + 1) % self.ring.len()];
                st.connections.insert((curr.clone(), Edge::Right), next.clone());
                st.connections.insert((next.clone(), Edge::Left), curr.clone());
            }
        }
        for ((src, edge), target) in &self.custom_links {
            st.connections.insert((src.clone(), *edge), target.clone());
        }
        st
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

    fn make_layout(id: &str, w: u32, h: u32) -> ScreenLayout {
        ScreenLayout {
            device_id: id.to_string(),
            name: id.to_string(),
            bounds: DisplayBounds::new(0, 0, w, h, 1.0, true),
        }
    }

    #[test]
    fn test_circular_topology_2_devices_right_and_left() {
        let mut ct = CircularTopology::from_ring(vec![
            make_layout("A", 1366, 768),
            make_layout("B", 1920, 1080),
        ]);

        assert_eq!(ct.len(), 2);
        assert_eq!(ct.active_device(), "A");

        // Right routing: A -> B -> A -> B
        assert_eq!(ct.right_neighbor("A"), Some("B"));
        assert_eq!(ct.right_neighbor("B"), Some("A"));

        // Left routing: A -> B -> A -> B
        assert_eq!(ct.left_neighbor("A"), Some("B"));
        assert_eq!(ct.left_neighbor("B"), Some("A"));

        // Transfer A.Right -> B.Left with y=0.35
        let t1 = ct.transfer("A", Edge::Right, NormalizedPoint { x: 1.0, y: 0.35 }).unwrap();
        assert_eq!(t1.source_device, "A");
        assert_eq!(t1.target_device, "B");
        assert_eq!(t1.edge, Edge::Right);
        assert_eq!(t1.entry_point.x, 0.0); // Enters B Left edge
        assert_eq!(t1.entry_point.y, 0.35); // Perpendicular y preserved
        assert_eq!(ct.active_device(), "B");

        // Transfer B.Right -> A.Left with y=0.82
        let t2 = ct.transfer("B", Edge::Right, NormalizedPoint { x: 1.0, y: 0.82 }).unwrap();
        assert_eq!(t2.source_device, "B");
        assert_eq!(t2.target_device, "A");
        assert_eq!(t2.entry_point.x, 0.0); // Enters A Left edge
        assert_eq!(t2.entry_point.y, 0.82); // Perpendicular y preserved
        assert_eq!(ct.active_device(), "A");

        // Now move LEFT from A: A.Left -> B.Right with y=0.55
        let t3 = ct.transfer("A", Edge::Left, NormalizedPoint { x: 0.0, y: 0.55 }).unwrap();
        assert_eq!(t3.source_device, "A");
        assert_eq!(t3.target_device, "B");
        assert_eq!(t3.entry_point.x, 1.0); // Enters B Right edge
        assert_eq!(t3.entry_point.y, 0.55); // Perpendicular y preserved
        assert_eq!(ct.active_device(), "B");

        // Move LEFT from B: B.Left -> A.Right with y=0.15
        let t4 = ct.transfer("B", Edge::Left, NormalizedPoint { x: 0.0, y: 0.15 }).unwrap();
        assert_eq!(t4.source_device, "B");
        assert_eq!(t4.target_device, "A");
        assert_eq!(t4.entry_point.x, 1.0); // Enters A Right edge
        assert_eq!(t4.entry_point.y, 0.15); // Perpendicular y preserved
        assert_eq!(ct.active_device(), "A");
    }

    #[test]
    fn test_circular_topology_3_devices_right_circle() {
        let mut ct = CircularTopology::from_ring(vec![
            make_layout("A", 1366, 768),
            make_layout("B", 1920, 1080),
            make_layout("C", 2560, 1440),
        ]);

        assert_eq!(ct.len(), 3);
        assert_eq!(ct.active_device(), "A");

        // RIGHT circle: A -> B -> C -> A -> B -> C
        assert_eq!(ct.right_neighbor("A"), Some("B"));
        assert_eq!(ct.right_neighbor("B"), Some("C"));
        assert_eq!(ct.right_neighbor("C"), Some("A"));

        // A -> B
        let t1 = ct.transfer("A", Edge::Right, NormalizedPoint { x: 1.0, y: 0.4 }).unwrap();
        assert_eq!(t1.target_device, "B");
        assert_eq!(t1.entry_point.x, 0.0);
        assert_eq!(t1.entry_point.y, 0.4);
        assert_eq!(ct.active_device(), "B");

        // B -> C
        let t2 = ct.transfer("B", Edge::Right, NormalizedPoint { x: 1.0, y: 0.6 }).unwrap();
        assert_eq!(t2.target_device, "C");
        assert_eq!(t2.entry_point.x, 0.0);
        assert_eq!(t2.entry_point.y, 0.6);
        assert_eq!(ct.active_device(), "C");

        // C -> A
        let t3 = ct.transfer("C", Edge::Right, NormalizedPoint { x: 1.0, y: 0.2 }).unwrap();
        assert_eq!(t3.target_device, "A");
        assert_eq!(t3.entry_point.x, 0.0);
        assert_eq!(t3.entry_point.y, 0.2);
        assert_eq!(ct.active_device(), "A");

        // Second loop: A -> B
        let t4 = ct.transfer("A", Edge::Right, NormalizedPoint { x: 1.0, y: 0.9 }).unwrap();
        assert_eq!(t4.target_device, "B");
        assert_eq!(t4.entry_point.x, 0.0);
        assert_eq!(t4.entry_point.y, 0.9);
        assert_eq!(ct.active_device(), "B");
    }

    #[test]
    fn test_circular_topology_3_devices_left_circle() {
        let mut ct = CircularTopology::from_ring(vec![
            make_layout("A", 1366, 768),
            make_layout("B", 1920, 1080),
            make_layout("C", 2560, 1440),
        ]);

        // LEFT circle: A -> C -> B -> A -> C -> B
        assert_eq!(ct.left_neighbor("A"), Some("C"));
        assert_eq!(ct.left_neighbor("C"), Some("B"));
        assert_eq!(ct.left_neighbor("B"), Some("A"));

        // A -> C
        let t1 = ct.transfer("A", Edge::Left, NormalizedPoint { x: 0.0, y: 0.3 }).unwrap();
        assert_eq!(t1.target_device, "C");
        assert_eq!(t1.entry_point.x, 1.0); // Enters C Right edge
        assert_eq!(t1.entry_point.y, 0.3);
        assert_eq!(ct.active_device(), "C");

        // C -> B
        let t2 = ct.transfer("C", Edge::Left, NormalizedPoint { x: 0.0, y: 0.7 }).unwrap();
        assert_eq!(t2.target_device, "B");
        assert_eq!(t2.entry_point.x, 1.0); // Enters B Right edge
        assert_eq!(t2.entry_point.y, 0.7);
        assert_eq!(ct.active_device(), "B");

        // B -> A
        let t3 = ct.transfer("B", Edge::Left, NormalizedPoint { x: 0.0, y: 0.5 }).unwrap();
        assert_eq!(t3.target_device, "A");
        assert_eq!(t3.entry_point.x, 1.0); // Enters A Right edge
        assert_eq!(t3.entry_point.y, 0.5);
        assert_eq!(ct.active_device(), "A");
    }

    #[test]
    fn test_circular_topology_n_devices() {
        // N = 5 devices
        let devices: Vec<ScreenLayout> = (0..5)
            .map(|i| make_layout(&format!("NODE_{}", i), 1920, 1080))
            .collect();
        let mut ct = CircularTopology::from_ring(devices);

        assert_eq!(ct.len(), 5);

        // Verify full circle right
        let mut curr = "NODE_0".to_string();
        for step in 1..=10 {
            let next_expected = format!("NODE_{}", step % 5);
            let t = ct.transfer(&curr, Edge::Right, NormalizedPoint { x: 1.0, y: 0.5 }).unwrap();
            assert_eq!(t.target_device, next_expected);
            assert_eq!(t.entry_point.x, 0.0);
            assert_eq!(t.entry_point.y, 0.5);
            curr = next_expected;
        }

        // Verify full circle left
        let mut curr = "NODE_0".to_string();
        for step in 1..=10 {
            let prev_expected = format!("NODE_{}", (50 - step) % 5);
            let t = ct.transfer(&curr, Edge::Left, NormalizedPoint { x: 0.0, y: 0.5 }).unwrap();
            assert_eq!(t.target_device, prev_expected);
            assert_eq!(t.entry_point.x, 1.0);
            assert_eq!(t.entry_point.y, 0.5);
            curr = prev_expected;
        }
    }

    #[test]
    fn test_legacy_topology_edge_transfer() {
        let mut topology = ScreenTopology::new();
        let dev_a = make_layout("PC_A", 1920, 1080);
        let dev_b = make_layout("PC_B", 2560, 1440);

        topology.add_device(dev_a);
        topology.add_device(dev_b);
        topology.connect("PC_A", Edge::Right, "PC_B");

        let router = InputRouter::new("PC_A", topology);

        let transfer = router.check_edge_transfer(1919, 540).unwrap();
        assert_eq!(transfer.source_device, "PC_A");
        assert_eq!(transfer.target_device, "PC_B");
        assert_eq!(transfer.edge, Edge::Right);
        assert_eq!(transfer.entry_point.x, 0.0);
        assert!((transfer.entry_point.y - 0.5).abs() < 0.05);
    }
}
