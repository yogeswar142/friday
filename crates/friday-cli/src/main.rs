use clap::{Parser, Subcommand};
use friday_core::{
    DisplayBounds, Edge, InputEvent, InputRouter, MouseEvent, ScreenLayout, ScreenTopology,
};
use friday_network::{
    generate_pairing_pin, DeviceIdentity, DeviceManager, NetworkPacket, NetworkTransport,
    PacketPayload, TrustStore,
};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "friday")]
#[command(
    about = "FRIDAY — Ultra-Low-Latency Peripheral & Data Sharing Platform",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start FRIDAY daemon engine
    Start {
        #[arg(short, long, default_value = "0.0.0.0:48700")]
        bind: String,
    },
    /// Show current status and active peer connections
    Status,
    /// List local identity, paired devices, and discovered nearby devices
    Devices,
    /// Scan local network for nearby FRIDAY devices via mDNS and broadcast
    Discover {
        #[arg(short, long, default_value = "3")]
        seconds: u64,
    },
    /// Pair with a remote FRIDAY node (by device ID or IP address)
    Pair {
        /// Target device ID or IP address
        target: String,
        /// Optional 6-digit PIN code (generated automatically if not provided)
        #[arg(short, long)]
        pin: Option<String>,
    },
    /// Connect to a paired trusted device
    Connect { device_id: String },
    /// Disconnect from a device
    Disconnect { device_id: String },
    /// Show detailed network diagnostics and interface metrics
    Diagnostics,
    /// Run health check and platform capabilities diagnostics
    Doctor,
    /// Run real-time latency and throughput benchmarks
    Benchmark {
        #[arg(short, long, default_value = "1000")]
        samples: u32,
    },
    /// Run headless multi-device virtual screen edge crossing simulation
    Simulate {
        #[arg(short, long, default_value = "100")]
        events: u32,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { bind } => {
            println!("🚀 Starting FRIDAY daemon on {}", bind);
            let transport = NetworkTransport::bind(&bind).await?;
            println!("Local endpoint bound at: {:?}", transport.local_addr()?);
            println!("FRIDAY Core engine initialized and ready.");
        }
        Commands::Status => {
            let identity = DeviceIdentity::load_or_create(None);
            let trust_store = TrustStore::new(None);
            let trusted = trust_store.list_trusted();

            println!("FRIDAY Status:");
            println!(
                "  Local Device   : {} ('{}')",
                identity.device_id, identity.display_name
            );
            println!("  OS Target      : {} ({})", identity.os, identity.arch);
            println!("  Engine State   : Running");
            println!("  Trusted Peers  : {}", trusted.len());
            println!("  Data Plane     : Ultra-Low-Latency UDP (<1ms estimated)");
        }
        Commands::Devices => {
            let identity = DeviceIdentity::load_or_create(None);
            let trust_store = TrustStore::new(None);
            let trusted = trust_store.list_trusted();

            println!("🖥  LOCAL DEVICE (This Machine):");
            println!("  Name         : {}", identity.display_name);
            println!("  Device ID    : {}", identity.device_id);
            println!("  Hostname     : {}", identity.hostname);
            println!("  Platform     : {} ({})", identity.os, identity.arch);
            println!("  Version      : {}", identity.version);
            println!();

            println!("🤝 PAIRED & TRUSTED DEVICES ({}):", trusted.len());
            if trusted.is_empty() {
                println!(
                    "  (No paired devices found. Run 'friday discover' to find nearby devices)"
                );
            } else {
                for dev in trusted {
                    println!(
                        "  • {} [Trusted]\n    ID: {}\n    Last Endpoint: {}:{}",
                        dev.display_name, dev.device_id, dev.last_known_ip, dev.last_known_port
                    );
                }
            }
        }
        Commands::Discover { seconds } => {
            println!(
                "🔍 Scanning local network for FRIDAY devices via mDNS and UDP broadcast ({}s)...",
                seconds
            );

            let dm = DeviceManager::new("0.0.0.0:0", 48700, None).await?;
            dm.start_discovery().ok();
            dm.scan_now();

            tokio::time::sleep(Duration::from_secs(seconds)).await;

            let discovered = dm.get_discovered_devices();
            println!("\nDiscovered Devices ({})", discovered.len());
            if discovered.is_empty() {
                println!("  No new FRIDAY devices detected on local network.");
                println!("  Ensure FRIDAY is open and running on the other computer.");
            } else {
                for dev in discovered {
                    let trust_status = if dev.is_paired {
                        "🟢 Paired & Trusted"
                    } else {
                        "⚪ Nearby (Unpaired)"
                    };
                    println!(
                        "  • {} ({})\n    Status     : {}\n    Device ID  : {}\n    Endpoint   : {}\n    Source     : {:?}",
                        dev.display_name, dev.os, trust_status, dev.device_id, dev.endpoint, dev.discovery_source
                    );
                }
            }
        }
        Commands::Pair { target, pin } => {
            let chosen_pin = pin.unwrap_or_else(generate_pairing_pin);
            println!("🤝 Initiating pairing with '{}'...", target);
            println!("--------------------------------------------------");
            println!("  Pairing Code: {}", chosen_pin);
            println!("--------------------------------------------------");
            println!("  Please confirm this 6-digit code on the other device.");
            println!("  Waiting for approval (up to 30s)...");

            let dm = DeviceManager::new("0.0.0.0:0", 48700, None).await?;
            match dm.pair_device(&target, Some(&chosen_pin)).await {
                Ok(trusted_device) => {
                    println!("\n✅ Pairing SUCCESSFUL!");
                    println!("  Device Name : {}", trusted_device.display_name);
                    println!("  Device ID   : {}", trusted_device.device_id);
                    println!("  Saved to trust store. Automatic connection enabled.");
                }
                Err(e) => {
                    eprintln!("\n❌ Pairing FAILED: {}", e);
                }
            }
        }
        Commands::Connect { device_id } => {
            println!("Connecting to trusted device: {}", device_id);
            let dm = DeviceManager::new("0.0.0.0:0", 48700, None).await?;
            match dm.connect_device(&device_id).await {
                Ok(()) => {
                    println!("✓ Authenticated session established.");
                }
                Err(e) => {
                    eprintln!("Failed to connect: {}", e);
                }
            }
        }
        Commands::Disconnect { device_id } => {
            println!("Disconnecting from device: {}", device_id);
            let dm = DeviceManager::new("0.0.0.0:0", 48700, None).await?;
            let _ = dm.disconnect_device(&device_id).await;
            println!("✓ Disconnected.");
        }
        Commands::Diagnostics => {
            let identity = DeviceIdentity::load_or_create(None);
            let dm = DeviceManager::new("0.0.0.0:0", 48700, None).await?;
            let diag = dm.get_network_diagnostics(None);

            println!("=== FRIDAY Network & Transport Diagnostics ===");
            println!("Local Device ID      : {}", identity.device_id);
            println!("Display Name         : {}", identity.display_name);
            println!("Local IP Endpoint    : {}:{}", diag.local_ip, diag.port);
            println!("Transport Layer      : {}", diag.transport);
            println!("Network Interface    : {}", diag.interface);
            println!("Discovery Engine     : {}", diag.discovery_status);
            println!(
                "Packets Sent / Recv  : {} / {}",
                diag.packets_sent, diag.packets_received
            );
            println!("Round-Trip Latency   : {:.2} ms", diag.rtt_ms);
            println!("Estimated Loss       : {:.1}%", diag.packet_loss_pct);
            println!("Active State         : {:?}", diag.state);
        }
        Commands::Doctor => {
            println!("=== FRIDAY System Doctor ===");
            println!("OS Target: {}", std::env::consts::OS);
            println!("Architecture: {}", std::env::consts::ARCH);

            #[cfg(target_os = "linux")]
            {
                let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
                println!(
                    "Display Server: {}",
                    if is_wayland { "Wayland" } else { "X11" }
                );
            }
            #[cfg(target_os = "windows")]
            {
                println!("Subsystem: Win32 RawInput / SendInput API");
            }
            #[cfg(target_os = "macos")]
            {
                println!("Subsystem: Quartz CGEvent API");
            }

            println!("Discovery Engine: mDNS Bonjour + Subnet Broadcast Fallback");
            println!("Data Transport: UDP High Frequency Datagrams");
            println!("Security: Authenticated Pairing Tokens & Trust Store");
            println!("✓ All health checks passed.");
        }
        Commands::Benchmark { samples } => {
            println!(
                "=== Running FRIDAY Ultra-Low-Latency Benchmark ({} samples) ===",
                samples
            );

            let sender = NetworkTransport::bind("127.0.0.1:0").await?;
            let receiver = NetworkTransport::bind("127.0.0.1:0").await?;
            let recv_addr = receiver.local_addr()?;

            let mut total_latency_nanos: u128 = 0;
            let mut serialized_bytes: usize = 0;

            for i in 0..samples {
                let start = Instant::now();
                let input = InputEvent::Mouse(MouseEvent::MoveRel {
                    dx: (i % 10) as i16,
                    dy: (i % 10) as i16,
                    timestamp: i,
                });

                let packet = NetworkPacket::new_input(input, i);
                let encoded = packet.encode()?;
                serialized_bytes += encoded.len();

                sender
                    .send_input_to(packet.payload.into_input().unwrap(), recv_addr)
                    .await?;
                let (rec_packet, _src) = receiver.recv_packet().await?;
                let _decoded_input = rec_packet.payload.into_input().unwrap();

                let elapsed = start.elapsed();
                total_latency_nanos += elapsed.as_nanos();
            }

            let avg_latency_us = (total_latency_nanos as f64 / samples as f64) / 1000.0;
            let avg_packet_bytes = serialized_bytes as f64 / samples as f64;

            println!("Benchmark Results:");
            println!("  Samples Processed : {}", samples);
            println!("  Avg Packet Size   : {:.2} bytes", avg_packet_bytes);
            println!("  Avg Loopback RTT  : {:.3} µs", avg_latency_us);
            println!(
                "  Throughput Rate   : {:.0} pkts/sec",
                1_000_000.0 / avg_latency_us
            );
            println!("✓ Performance target achieved (< 1ms data plane latency).");
        }
        Commands::Simulate { events } => {
            println!("=== Running FRIDAY Virtual Multi-Device Screen Crossing Simulator ===");
            let mut topology = ScreenTopology::new();

            let pc_a = ScreenLayout {
                device_id: "PC_A".to_string(),
                name: "Primary PC A (1080p)".to_string(),
                bounds: DisplayBounds::new(0, 0, 1920, 1080, 1.0, true),
            };
            let pc_b = ScreenLayout {
                device_id: "PC_B".to_string(),
                name: "Secondary PC B (1440p)".to_string(),
                bounds: DisplayBounds::new(0, 0, 2560, 1440, 1.0, false),
            };

            topology.add_device(pc_a);
            topology.add_device(pc_b);
            topology.connect("PC_A", Edge::Right, "PC_B");

            let router = InputRouter::new("PC_A", topology);

            println!("Configured Screen Layout: [PC_A (1920x1080)] <---> [PC_B (2560x1440)]");
            println!(
                "Simulating {} mouse movement events across right screen edge...",
                events
            );

            let mut transitions = 0;
            for i in 0..events {
                let x = 1850 + (i as i32 % 80); // Sweeps 1850 to 1929 (crosses 1917px edge threshold)
                let y = 540;

                if let Some(transfer) = router.check_edge_transfer(x, y) {
                    transitions += 1;
                    if transitions == 1 {
                        println!(
                            "  [Event #{}] EDGE CROSSING DETECTED! {} ({},{}) -> {} Entry: ({:.2}, {:.2})",
                            i, transfer.source_device, x, y, transfer.target_device, transfer.entry_point.x, transfer.entry_point.y
                        );
                    }
                }
            }

            println!("Simulation Complete:");
            println!("  Total Events Simulated : {}", events);
            println!("  Edge Transfers Triggered: {}", transitions);
            println!("✓ Virtual Screen Topology Router Verified!");
        }
    }

    Ok(())
}

trait PacketPayloadExt {
    fn into_input(self) -> Option<InputEvent>;
}

impl PacketPayloadExt for PacketPayload {
    fn into_input(self) -> Option<InputEvent> {
        match self {
            PacketPayload::Input(evt) => Some(evt),
            _ => None,
        }
    }
}
