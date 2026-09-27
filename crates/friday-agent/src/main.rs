/// FRIDAY Agent — Phase 1.5 Real Hardware Validation Binary
///
/// Usage:
///   # On the RECEIVER (Yoga/Windows) — listen for incoming control:
///   friday-agent receive --bind 0.0.0.0:48700
///
///   # On the SENDER (G50/Linux) — capture and send to Yoga:
///   friday-agent send --peer 192.168.1.X:48700 --bind 0.0.0.0:48701
///
///   # Emergency escape (run on sender):
///   Press Ctrl+Alt+Shift+Escape (handled by agent)
///
/// Architecture:
///   SENDER MODE: capture XQueryPointer → detect edge → UDP → peer
///   RECEIVER MODE: UDP → XTestFakeEvent / SendInput
///   HANDOFF: when edge detected, send HandoffControl → peer becomes sender

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
};

use clap::{Parser, Subcommand};
use friday_agent::{
    control::{ControlMessage, EdgeTrigger, ScreenEdge, ScreenInfo},
    session::{receiver_loop, sender_loop, SessionHandle, SessionMode},
};
use friday_network::{NetworkPacket, NetworkTransport};
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const DEFAULT_PORT: u16 = 48700;

#[derive(Parser)]
#[command(name = "friday-agent")]
#[command(about = "FRIDAY Phase 1.5 — Real Mouse Sharing Agent")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Receiver mode: inject incoming events into this machine's OS
    Receive {
        /// UDP address to listen on
        #[arg(short, long, default_value = "0.0.0.0:48700")]
        bind: String,
    },
    /// Sender mode: capture local mouse and send to peer receiver
    Send {
        /// Receiver peer address (e.g. 192.168.1.50:48700)
        #[arg(short, long)]
        peer: String,
        /// Local bind address
        #[arg(short, long, default_value = "0.0.0.0:48701")]
        bind: String,
        /// Immediately start controlling remote cursor without waiting for edge
        #[arg(short, long, default_value_t = false)]
        direct: bool,
    },
    /// Full session mode: connect two instances, handle circular edge transfer
    Connect {
        /// Remote peer address
        #[arg(short, long)]
        peer: String,
        /// Local bind address
        #[arg(short, long, default_value = "0.0.0.0:48700")]
        bind: String,
        /// Edge detection threshold in pixels
        #[arg(short, long, default_value = "3")]
        edge_px: i32,
    },
    /// Print platform diagnostics
    Info,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Info => run_info().await,
        Commands::Receive { bind } => run_receiver(&bind).await,
        Commands::Send { peer, bind, direct } => run_sender(&bind, &peer, direct).await,
        Commands::Connect { peer, bind, edge_px } => {
            run_connect(&bind, &peer, edge_px).await
        }
    }
}

// ── Info ─────────────────────────────────────────────────────────────────────

async fn run_info() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== FRIDAY Agent Platform Info ===");
    println!("OS:   {}", std::env::consts::OS);
    println!("Arch: {}", std::env::consts::ARCH);

    #[cfg(target_os = "linux")]
    {
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| "(not set)".into());
        let wayland = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "(not set)".into());
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "(not set)".into());
        println!("DISPLAY:          {}", display);
        println!("WAYLAND_DISPLAY:  {}", wayland);
        println!("XDG_SESSION_TYPE: {}", session);

        match friday_agent::platform::linux::query_display_info() {
            Ok(info) => {
                println!(
                    "Screen:           {}x{} @ scale {:.2}",
                    info.width, info.height, info.scale_factor
                );
            }
            Err(e) => println!("Screen query failed: {}", e),
        }

        match friday_agent::platform::linux::get_cursor_position() {
            Ok((x, y)) => println!("Cursor position:  ({}, {})", x, y),
            Err(e) => println!("Cursor query failed: {}", e),
        }
    }

    #[cfg(target_os = "windows")]
    {
        match friday_agent::platform::windows::query_display_info() {
            Ok(info) => println!("Screen: {}x{}", info.width, info.height),
            Err(e) => println!("Screen query failed: {}", e),
        }
    }

    Ok(())
}

// ── Receiver ─────────────────────────────────────────────────────────────────

async fn run_receiver(bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("FRIDAY Agent — RECEIVER mode");
    info!("Binding on: {}", bind_addr);

    let transport = Arc::new(NetworkTransport::bind(bind_addr).await?);
    let local_addr = transport.local_addr()?;
    info!("Listening on {}", local_addr);

    let (screen_w, screen_h) = get_screen_size();
    let session = SessionHandle::new(screen_w, screen_h);

    // Start receiver loop in background
    let transport_clone = transport.clone();
    let session_clone = session.clone();
    tokio::spawn(async move {
        receiver_loop(transport_clone, session_clone).await;
    });

    println!("✓ FRIDAY Receiver ready on {}", local_addr);
    println!("  Waiting for sender to connect...");
    println!("  Press Ctrl+C to stop.");

    // Keep main alive
    tokio::signal::ctrl_c().await?;
    info!("Shutdown requested.");
    session.request_stop();

    Ok(())
}

// ── Sender ────────────────────────────────────────────────────────────────────

async fn run_sender(bind_addr: &str, peer_addr: &str, direct: bool) -> Result<(), Box<dyn std::error::Error>> {
    info!("FRIDAY Agent — SENDER mode");
    info!("Binding on: {}", bind_addr);
    info!("Peer: {}", peer_addr);

    let peer: SocketAddr = peer_addr.parse()?;
    let transport = Arc::new(NetworkTransport::bind(bind_addr).await?);

    let (screen_w, screen_h) = get_screen_size();
    let session = SessionHandle::new(screen_w, screen_h);
    session.set_peer(peer).await;
    session.set_mode(SessionMode::Sender).await;

    let is_remote_active = Arc::new(AtomicBool::new(direct));

    // Input event channel: capture → sender_loop
    let (input_tx, input_rx) = mpsc::channel::<friday_core::InputEvent>(256);

    // Edge trigger channel
    let (edge_tx, mut edge_rx) = mpsc::channel::<EdgeTrigger>(16);

    let seq = Arc::new(AtomicU32::new(0));

    // Start sender loop
    let tx_clone = transport.clone();
    let session_clone = session.clone();
    let seq_clone = seq.clone();
    tokio::spawn(async move {
        sender_loop(input_rx, tx_clone, session_clone, seq_clone).await;
    });

    // Start Linux capture loop in dedicated OS thread for ultra-low latency
    #[cfg(target_os = "linux")]
    {
        let stop = session.stop.clone();
        let remote_active_clone = is_remote_active.clone();
        std::thread::spawn(move || {
            friday_agent::platform::linux::capture_loop(
                input_tx,
                stop,
                remote_active_clone,
                4, // edge threshold pixels
                screen_w,
                screen_h,
                edge_tx,
            );
        });
    }

    // Send Hello handshake
    let hello = ControlMessage::Hello {
        device_name: hostname(),
        screen: ScreenInfo {
            width: screen_w as u32,
            height: screen_h as u32,
            scale_factor: 1.0,
            device_name: hostname(),
        },
    };
    let hello_bytes = hello.encode()?;
    let hello_packet = NetworkPacket::new_control(hello_bytes);
    let encoded = hello_packet.encode().map_err(|e| e.to_string())?;
    transport.send_raw_to(&encoded, peer).await.map_err(|e| e.to_string())?;
    info!("Sent Hello to {}", peer);

    // Edge handoff handler: when edge hit, activate remote control
    let is_remote_active_edge = is_remote_active.clone();
    tokio::spawn(async move {
        while let Some(trigger) = edge_rx.recv().await {
            info!(
                "Screen edge {:?} reached at ({:.2}, {:.2})! Transferring mouse to Yoga!",
                trigger.edge, trigger.norm_x, trigger.norm_y
            );
            is_remote_active_edge.store(true, Ordering::SeqCst);
        }
    });

    // Also start receiver loop to process pong / peer messages
    let transport_recv = transport.clone();
    let session_recv = session.clone();
    tokio::spawn(async move {
        receiver_loop(transport_recv, session_recv).await;
    });

    if direct {
        println!("🚀 DIRECT CONTROL ACTIVE: Moving your mouse on G50 moves the cursor on Yoga!");
    } else {
        println!("✓ FRIDAY Sender started.");
        println!("  Move mouse to any screen edge to transfer control to Yoga.");
    }
    println!("  Peer: {}", peer_addr);
    println!("  Press Ctrl+C to stop safely.");

    tokio::signal::ctrl_c().await?;
    session.request_stop();

    // Send goodbye
    if let Some(p) = session.get_peer().await {
        let bye = ControlMessage::Goodbye;
        if let Ok(bytes) = bye.encode() {
            let packet = NetworkPacket::new_control(bytes);
            if let Ok(enc) = packet.encode() {
                let _ = transport.send_raw_to(&enc, p).await;
            }
        }
    }

    info!("Shutdown complete.");
    Ok(())
}

// ── Connect (full circular session) ──────────────────────────────────────────

async fn run_connect(
    bind_addr: &str,
    peer_addr: &str,
    edge_px: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("FRIDAY Agent — CONNECT mode (circular edge handoff)");
    info!("Bind:  {}", bind_addr);
    info!("Peer:  {}", peer_addr);

    let peer: SocketAddr = peer_addr.parse()?;
    let transport = Arc::new(NetworkTransport::bind(bind_addr).await?);
    let local_addr = transport.local_addr()?;
    info!("Socket bound at {}", local_addr);

    let (screen_w, screen_h) = get_screen_size();
    info!("Local screen: {}x{}", screen_w, screen_h);

    let session = SessionHandle::new(screen_w, screen_h);
    session.set_peer(peer).await;
    // Start as Sender (G50 = primary sender until first edge crossing)
    session.set_mode(SessionMode::Sender).await;

    let (input_tx, input_rx) = mpsc::channel::<friday_core::InputEvent>(512);
    let (edge_tx, mut edge_rx) = mpsc::channel::<EdgeTrigger>(32);
    let seq = Arc::new(AtomicU32::new(0));

    // ── Sender loop ──
    let tx_cl = transport.clone();
    let sess_cl = session.clone();
    let seq_cl = seq.clone();
    tokio::spawn(async move {
        sender_loop(input_rx, tx_cl, sess_cl, seq_cl).await;
    });

    let is_remote_active = Arc::new(AtomicBool::new(false));

    // ── Linux capture loop in dedicated OS thread ──
    #[cfg(target_os = "linux")]
    {
        let stop = session.stop.clone();
        let remote_active_clone = is_remote_active.clone();
        std::thread::spawn(move || {
            friday_agent::platform::linux::capture_loop(
                input_tx,
                stop,
                remote_active_clone,
                edge_px,
                screen_w,
                screen_h,
                edge_tx,
            );
        });
    }

    // ── Edge handoff handler ──
    let trans_edge = transport.clone();
    let sess_edge = session.clone();
    tokio::spawn(async move {
        while let Some(trigger) = edge_rx.recv().await {
            if sess_edge.get_mode().await == SessionMode::Sender {
                info!(
                    "Edge {:?} at ({:.3}, {:.3}) → handing off to peer",
                    trigger.edge, trigger.norm_x, trigger.norm_y
                );
                let entry_x = match trigger.edge {
                    ScreenEdge::Right => 0.0,
                    ScreenEdge::Left => 1.0,
                    ScreenEdge::Top => trigger.norm_x,
                    ScreenEdge::Bottom => trigger.norm_x,
                };
                let entry_y = match trigger.edge {
                    ScreenEdge::Right
                    | ScreenEdge::Left => trigger.norm_y,
                    ScreenEdge::Top => 0.0,
                    ScreenEdge::Bottom => 1.0,
                };

                let msg = ControlMessage::HandoffControl {
                    entry_x_norm: entry_x,
                    entry_y_norm: entry_y,
                };
                if let Ok(bytes) = msg.encode() {
                    let packet = NetworkPacket::new_control(bytes);
                    if let Ok(enc) = packet.encode() {
                        if let Some(p) = sess_edge.get_peer().await {
                            if let Err(e) = trans_edge.send_raw_to(&enc, p).await {
                                warn!("HandoffControl send failed: {}", e);
                            }
                        }
                    }
                }
                sess_edge.set_mode(SessionMode::Receiver).await;
                info!("Switched to Receiver mode — peer has control.");
            }
        }
    });

    // ── Receiver loop (handles both injecting remote events AND control messages) ──
    let trans_recv = transport.clone();
    let sess_recv = session.clone();
    tokio::spawn(async move {
        receiver_loop(trans_recv, sess_recv).await;
    });

    // ── Send Hello ──
    let hello = ControlMessage::Hello {
        device_name: hostname(),
        screen: ScreenInfo {
            width: screen_w as u32,
            height: screen_h as u32,
            scale_factor: 1.0,
            device_name: hostname(),
        },
    };
    let hello_bytes = hello.encode()?;
    let packet = NetworkPacket::new_control(hello_bytes);
    let enc = packet.encode().map_err(|e| e.to_string())?;
    transport.send_raw_to(&enc, peer).await.map_err(|e| e.to_string())?;
    info!("Sent Hello to peer at {}", peer);

    println!("✓ FRIDAY Connect established.");
    println!("  Local:  {} ({}x{})", local_addr, screen_w, screen_h);
    println!("  Peer:   {}", peer_addr);
    println!("  Move mouse to any screen edge to transfer control.");
    println!("  Press Ctrl+C to stop safely.");

    tokio::signal::ctrl_c().await?;
    session.request_stop();

    // Send goodbye + ReleaseAll for safety
    if let Some(p) = session.get_peer().await {
        for msg in [ControlMessage::ReleaseAll, ControlMessage::Goodbye] {
            if let Ok(bytes) = msg.encode() {
                let packet = NetworkPacket::new_control(bytes);
                if let Ok(enc) = packet.encode() {
                    let _ = transport.send_raw_to(&enc, p).await;
                }
            }
        }
    }

    info!("FRIDAY agent stopped safely.");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_screen_size() -> (i32, i32) {
    #[cfg(target_os = "linux")]
    {
        friday_agent::platform::linux::query_display_info()
            .map(|d| (d.width as i32, d.height as i32))
            .unwrap_or((1920, 1080))
    }
    #[cfg(target_os = "windows")]
    {
        friday_agent::platform::windows::query_display_info()
            .map(|d| (d.width as i32, d.height as i32))
            .unwrap_or((1920, 1080))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        (1920, 1080)
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "unknown".to_string())
}
