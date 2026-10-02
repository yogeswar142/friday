use crate::types::{BenchmarkReportDto, DiagnosticCheckItem, DiagnosticReportDto};
use friday_core::{
    CircularTopology, DisplayBounds, Edge, InputEvent, MouseEvent, NormalizedPoint, ScreenLayout,
};
use std::time::Instant;

pub fn run_system_diagnostics(
    engine_state: &str,
    active_device: &str,
    ring_nodes: &[String],
    latency_ms: f32,
    packet_loss_pct: f32,
) -> DiagnosticReportDto {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let display_server = if cfg!(target_os = "linux") {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            "Wayland"
        } else if std::env::var("DISPLAY").is_ok() {
            "X11"
        } else {
            "Headless / Linux TTY"
        }
    } else if cfg!(target_os = "windows") {
        "Desktop Window Manager (Win32)"
    } else if cfg!(target_os = "macos") {
        "Quartz Compositor (macOS)"
    } else {
        "Unknown"
    };

    let input_backend = if cfg!(target_os = "linux") {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            "Wayland Input Capture (Portal)"
        } else {
            "X11 XTest / XRecord"
        }
    } else if cfg!(target_os = "windows") {
        "Win32 SendInput / RawInput Hook"
    } else if cfg!(target_os = "macos") {
        "CoreGraphics Event Tap / CGEvent"
    } else {
        "Generic Mock Platform"
    };

    let mut checks = Vec::new();

    // Check 1: Engine thread state
    checks.push(DiagnosticCheckItem {
        name: "Engine Control Plane".into(),
        passed: engine_state == "running",
        detail: format!("Engine is {}", engine_state),
    });

    // Check 2: Ring topology sanity
    let topo_valid = ring_nodes.len() >= 2;
    checks.push(DiagnosticCheckItem {
        name: "Circular Topology Ring".into(),
        passed: topo_valid,
        detail: if topo_valid {
            format!(
                "{} nodes in circular ring: {}",
                ring_nodes.len(),
                ring_nodes.join(" → ")
            )
        } else {
            "Need at least 2 connected devices in circular ring".into()
        },
    });

    // Check 3: Active Device
    let active_valid = !active_device.is_empty() && ring_nodes.contains(&active_device.to_string());
    checks.push(DiagnosticCheckItem {
        name: "Active Input Owner".into(),
        passed: active_valid,
        detail: if active_valid {
            format!("Exclusive ownership assigned to {}", active_device)
        } else {
            "No active node elected in ring".into()
        },
    });

    // Check 4: Network Loopback / Ping
    let net_ok = latency_ms >= 0.0 && packet_loss_pct < 5.0;
    checks.push(DiagnosticCheckItem {
        name: "Network Latency & Jitter".into(),
        passed: net_ok,
        detail: format!(
            "{:.2} ms roundtrip latency, {:.1}% packet loss",
            latency_ms, packet_loss_pct
        ),
    });

    // Check 5: Display Server Access
    let display_ok = !display_server.contains("Unknown");
    checks.push(DiagnosticCheckItem {
        name: "Platform Display Server".into(),
        passed: display_ok,
        detail: format!("Connected to {}", display_server),
    });

    DiagnosticReportDto {
        engine_state: engine_state.to_string(),
        platform: os.to_string(),
        architecture: arch.to_string(),
        os_version: std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_else(|_| display_server.to_string()),
        input_backend: input_backend.to_string(),
        network_transport: "UDP with Bincode packet protocol (sub-millisecond)".into(),
        latency_ms,
        packet_loss_pct,
        active_device: active_device.to_string(),
        ring_nodes: ring_nodes.to_vec(),
        topology_valid: topo_valid,
        checks,
    }
}

pub fn run_core_benchmark() -> BenchmarkReportDto {
    // Benchmark 1: Serialization throughput
    let iterations = 100_000;
    let sample_event = InputEvent::Mouse(MouseEvent::MoveRel {
        dx: 12,
        dy: -8,
        timestamp: 123456,
    });

    let t0 = Instant::now();
    for _ in 0..iterations {
        let bytes = bincode::serialize(&sample_event).unwrap();
        let _de: InputEvent = bincode::deserialize(&bytes).unwrap();
    }
    let elapsed_sec = t0.elapsed().as_secs_f64();
    let kops = (iterations as f64 / elapsed_sec) / 1000.0;

    // Benchmark 2: Coordinate normalization
    let bounds = DisplayBounds::new(0, 0, 1920, 1080, 1.0, true);
    let t1 = Instant::now();
    for i in 0..iterations {
        let x = i % 1920;
        let y = i % 1080;
        let _ = bounds.to_normalized(x, y);
    }
    let coord_ns = (t1.elapsed().as_nanos() as f64) / (iterations as f64);

    // Benchmark 3: Circular routing decisions
    let mut ct = CircularTopology::from_ring(vec![
        ScreenLayout {
            device_id: "PC_A".into(),
            name: "PC A".into(),
            bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
        },
        ScreenLayout {
            device_id: "PC_B".into(),
            name: "PC B".into(),
            bounds: DisplayBounds::new(0, 0, 2560, 1440, 1.0, false),
        },
        ScreenLayout {
            device_id: "PC_C".into(),
            name: "PC C".into(),
            bounds: DisplayBounds::new(0, 0, 1366, 768, 1.0, false),
        },
    ]);

    let t2 = Instant::now();
    let mut curr = "PC_A";
    let hops = 50_000;
    for _ in 0..hops {
        if let Some(t) = ct.transfer(curr, Edge::Right, NormalizedPoint { x: 1.0, y: 0.5 }) {
            curr = if t.target_device == "PC_A" {
                "PC_A"
            } else if t.target_device == "PC_B" {
                "PC_B"
            } else {
                "PC_C"
            };
        }
    }
    let routing_ns = (t2.elapsed().as_nanos() as f64) / (hops as f64);

    let rating = if kops > 500.0 && routing_ns < 1000.0 {
        "Ultra Low Latency (Production Ready)"
    } else {
        "High Performance"
    };

    BenchmarkReportDto {
        serialization_throughput_kops: kops,
        coord_transform_latency_ns: coord_ns,
        routing_decision_latency_ns: routing_ns,
        simulated_hops_tested: hops,
        rating: rating.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_diagnostics_evaluation() {
        let nodes = vec!["G50".to_string(), "Yoga".to_string()];
        let report = run_system_diagnostics("running", "G50", &nodes, 0.5, 0.0);

        assert_eq!(report.engine_state, "running");
        assert_eq!(report.active_device, "G50");
        assert!(report.topology_valid);
        assert_eq!(report.checks.len(), 5);
        for check in &report.checks {
            assert!(
                check.passed,
                "Check '{}' failed: {}",
                check.name, check.detail
            );
        }
    }

    #[test]
    fn test_core_benchmark_execution() {
        let report = run_core_benchmark();
        assert!(report.serialization_throughput_kops > 0.0);
        assert!(report.coord_transform_latency_ns > 0.0);
        assert!(report.routing_decision_latency_ns > 0.0);
        assert_eq!(report.simulated_hops_tested, 50_000);
        assert!(!report.rating.is_empty());
    }
}
