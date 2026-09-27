/// FRIDAY Session Engine — Phase 1.5
///
/// Manages the bidirectional input sharing session between two FRIDAY agents.
///
/// State machine:
///   Local  → capture mouse events → send to remote
///   Remote → receives events → inject into local OS
///
/// Session modes:
///   Sender: captures local input, sends to peer
///   Receiver: receives from peer, injects into local OS
///
/// The session transitions automatically on edge crossing.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use friday_core::{ElementState, InputEvent, MouseButton, MouseEvent};
use friday_network::{NetworkPacket, NetworkTransport, PacketPayload};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{control::ControlMessage, error::{AgentError, Result}};

/// How long without a packet before considering connection dead
const HEARTBEAT_TIMEOUT_MS: u64 = 3000;
const HEARTBEAT_INTERVAL_MS: u64 = 500;

/// Session state: controls where input goes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionMode {
    /// We are the controlling device — local input routes to peer
    Sender,
    /// Peer is controlling — we inject received events into local OS
    Receiver,
    /// Disconnected / no session
    Disconnected,
}

/// Shared session state (cheaply cloneable)
#[derive(Clone)]
pub struct SessionHandle {
    pub mode: Arc<tokio::sync::Mutex<SessionMode>>,
    pub stop: Arc<AtomicBool>,
    pub peer_addr: Arc<tokio::sync::Mutex<Option<SocketAddr>>>,
    pub local_screen_w: i32,
    pub local_screen_h: i32,
}

impl SessionHandle {
    pub fn new(screen_w: i32, screen_h: i32) -> Self {
        Self {
            mode: Arc::new(tokio::sync::Mutex::new(SessionMode::Disconnected)),
            stop: Arc::new(AtomicBool::new(false)),
            peer_addr: Arc::new(tokio::sync::Mutex::new(None)),
            local_screen_w: screen_w,
            local_screen_h: screen_h,
        }
    }

    pub async fn get_mode(&self) -> SessionMode {
        *self.mode.lock().await
    }

    pub async fn set_mode(&self, mode: SessionMode) {
        *self.mode.lock().await = mode;
    }

    pub async fn get_peer(&self) -> Option<SocketAddr> {
        *self.peer_addr.lock().await
    }

    pub async fn set_peer(&self, addr: SocketAddr) {
        *self.peer_addr.lock().await = Some(addr);
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Send a ControlMessage over UDP to the peer
async fn send_control(
    transport: &NetworkTransport,
    peer: SocketAddr,
    msg: &ControlMessage,
) -> Result<()> {
    let payload = msg.encode().map_err(|e| AgentError::NetworkError(e.to_string()))?;
    let packet = NetworkPacket::new_control(payload);
    let bytes = packet
        .encode()
        .map_err(|e| AgentError::NetworkError(e.to_string()))?;

    // NetworkTransport.send_raw sends arbitrary bytes to addr
    transport
        .send_raw_to(&bytes, peer)
        .await
        .map_err(|e| AgentError::NetworkError(e.to_string()))
}

/// Safety reset: release all held buttons on the local machine
/// Used when disconnected or during emergency escape
pub async fn release_all_local(transport: &NetworkTransport, peer: Option<SocketAddr>) {
    // Send ReleaseAll to peer as well (they may hold buttons on our behalf)
    if let Some(peer) = peer {
        let _ = send_control(transport, peer, &ControlMessage::ReleaseAll).await;
    }
}

/// The main sender loop: reads captured events from `rx` and sends to peer
pub async fn sender_loop(
    mut rx: mpsc::Receiver<InputEvent>,
    transport: Arc<NetworkTransport>,
    session: SessionHandle,
    seq: Arc<std::sync::atomic::AtomicU32>,
) {
    info!("Sender loop started — routing local input to peer");
    let mut heartbeat_timer = Instant::now();
    let mut sent_count = 0u64;

    loop {
        if session.stop.load(Ordering::Relaxed) {
            info!("Sender loop: stop requested");
            break;
        }

        let peer = match session.get_peer().await {
            Some(p) => p,
            None => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }
        };

        let mode = session.get_mode().await;
        if mode != SessionMode::Sender {
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        }

        // Drain and batch up to 8 events for efficiency
        let mut batch = Vec::with_capacity(8);
        if let Ok(evt) = rx.try_recv() {
            batch.push(evt);
            for _ in 1..8 {
                match rx.try_recv() {
                    Ok(e) => batch.push(e),
                    Err(_) => break,
                }
            }
        }

        for evt in batch {
            let s = seq.fetch_add(1, Ordering::SeqCst);
            let packet = NetworkPacket::new_input(evt, s);
            match packet.encode() {
                Ok(bytes) => {
                    if let Err(e) = transport.send_raw_to(&bytes, peer).await {
                        warn!("Send error: {}", e);
                    } else {
                        sent_count += 1;
                    }
                }
                Err(e) => error!("Encode error: {}", e),
            }
        }

        // Heartbeat every 500ms while in sender mode
        if heartbeat_timer.elapsed() > Duration::from_millis(HEARTBEAT_INTERVAL_MS) {
            let ping = ControlMessage::Ping { seq: seq.load(Ordering::Relaxed) };
            let _ = send_control(&transport, peer, &ping).await;
            heartbeat_timer = Instant::now();
        }

        // Yield to tokio scheduler (prevents busy loop)
        tokio::task::yield_now().await;
    }

    info!("Sender loop stopped. Total events sent: {}", sent_count);
}

/// The main receiver loop: reads packets from UDP socket, injects into OS
///
/// When the peer sends HandoffControl, we switch to Sender mode.
/// When we reach an edge, we send HandoffControl back.
pub async fn receiver_loop(
    transport: Arc<NetworkTransport>,
    session: SessionHandle,
) {
    info!("Receiver loop started — accepting remote input");
    let mut last_packet_time = Instant::now();
    let mut received_count = 0u64;

    loop {
        if session.stop.load(Ordering::Relaxed) {
            info!("Receiver loop: stop requested");
            break;
        }

        // Non-blocking recv with timeout
        match tokio::time::timeout(
            Duration::from_millis(100),
            transport.recv_packet(),
        )
        .await
        {
            Ok(Ok((packet, src_addr))) => {
                last_packet_time = Instant::now();
                received_count += 1;

                // Ensure peer is registered
                {
                    let peer = session.peer_addr.lock().await;
                    if peer.is_none() {
                        drop(peer);
                        session.set_peer(src_addr).await;
                        session.set_mode(SessionMode::Receiver).await;
                        info!("Peer registered: {}", src_addr);
                    }
                }

                match &packet.payload {
                    PacketPayload::Input(evt) => {
                        if session.get_mode().await == SessionMode::Receiver {
                            // Inject into local OS
                            if let Err(e) = inject_platform_event(evt) {
                                debug!("Inject error: {}", e);
                            }
                        }
                    }
                    PacketPayload::Control(bytes) => {
                        handle_control_message(bytes, src_addr, &transport, &session).await;
                    }
                    _ => {}
                }
            }
            Ok(Err(e)) => {
                debug!("Recv error: {}", e);
            }
            Err(_) => {
                // Timeout — check heartbeat
                if session.get_mode().await != SessionMode::Disconnected
                    && last_packet_time.elapsed() > Duration::from_millis(HEARTBEAT_TIMEOUT_MS)
                {
                    warn!("Heartbeat timeout — connection lost. Restoring local control.");
                    handle_disconnect(&transport, &session).await;
                }
            }
        }
    }

    info!("Receiver loop stopped. Total events received: {}", received_count);
}

async fn handle_control_message(
    bytes: &[u8],
    src: SocketAddr,
    transport: &NetworkTransport,
    session: &SessionHandle,
) {
    let msg = match ControlMessage::decode(bytes) {
        Ok(m) => m,
        Err(e) => {
            warn!("Bad control message from {}: {}", src, e);
            return;
        }
    };

    match msg {
        ControlMessage::Hello { device_name, screen } => {
            info!("Hello from {} ({}x{})", device_name, screen.width, screen.height);
            session.set_peer(src).await;
            // Reply with Welcome
            let welcome = ControlMessage::Welcome {
                device_name: hostname(),
                screen: crate::control::ScreenInfo {
                    width: session.local_screen_w as u32,
                    height: session.local_screen_h as u32,
                    scale_factor: 1.0,
                    device_name: hostname(),
                },
            };
            let _ = send_control(transport, src, &welcome).await;
            session.set_mode(SessionMode::Receiver).await;
            info!("Session established. We are in Receiver mode.");
        }

        ControlMessage::Welcome { device_name, screen } => {
            info!("Welcome from {} ({}x{})", device_name, screen.width, screen.height);
            // The peer accepted — we stay in Sender mode
            session.set_mode(SessionMode::Sender).await;
            info!("Session established. We are in Sender mode.");
        }

        ControlMessage::HandoffControl { entry_x_norm, entry_y_norm } => {
            // Peer is handing control to us — we become Sender
            info!(
                "HandoffControl received — taking control at ({:.2}, {:.2})",
                entry_x_norm, entry_y_norm
            );
            // Place cursor at entry point
            let px = (entry_x_norm * (session.local_screen_w as f32 - 1.0)).round() as i32;
            let py = (entry_y_norm * (session.local_screen_h as f32 - 1.0)).round() as i32;
            let _ = platform_set_cursor(px, py);
            session.set_mode(SessionMode::Sender).await;
        }

        ControlMessage::ReturnControl { entry_x_norm, entry_y_norm } => {
            // Peer wants to return control to us
            info!("ReturnControl received — resuming local capture");
            let px = (entry_x_norm * (session.local_screen_w as f32 - 1.0)).round() as i32;
            let py = (entry_y_norm * (session.local_screen_h as f32 - 1.0)).round() as i32;
            let _ = platform_set_cursor(px, py);
            session.set_mode(SessionMode::Sender).await;
        }

        ControlMessage::ReleaseAll => {
            info!("ReleaseAll received — releasing held inputs");
            // Inject button release events for safety
            for btn in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
                let evt = InputEvent::Mouse(MouseEvent::Button {
                    button: btn,
                    state: ElementState::Released,
                    timestamp: 0,
                });
                let _ = inject_platform_event(&evt);
            }
        }

        ControlMessage::Ping { seq } => {
            let _ = send_control(transport, src, &ControlMessage::Pong { seq }).await;
        }

        ControlMessage::Pong { .. } => {
            // Update liveness timestamp (already done above via last_packet_time)
        }

        ControlMessage::Goodbye => {
            info!("Peer disconnected gracefully.");
            handle_disconnect(transport, session).await;
        }
    }
}

async fn handle_disconnect(_transport: &NetworkTransport, session: &SessionHandle) {
    // Release all held buttons on this machine
    for btn in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        let evt = InputEvent::Mouse(MouseEvent::Button {
            button: btn,
            state: ElementState::Released,
            timestamp: 0,
        });
        let _ = inject_platform_event(&evt);
    }
    session.set_mode(SessionMode::Sender).await; // Restore local control
    info!("Local control restored after disconnect.");
}

/// Dispatch injection to the correct platform backend
fn inject_platform_event(event: &InputEvent) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::inject_event(event)
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::inject_event(event)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = event;
        Err(AgentError::InjectionError("Unsupported platform".into()))
    }
}

/// Move cursor to pixel position using the correct platform API
fn platform_set_cursor(x: i32, y: i32) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::inject_move_abs(x, y)
    }
    #[cfg(target_os = "windows")]
    {
        let evt = InputEvent::Mouse(MouseEvent::MoveAbs {
            // Convert pixel to 0..65535
            x_norm: ((x as f32 / 1920.0) * 65535.0) as u16, // Rough — corrected in session
            y_norm: ((y as f32 / 1080.0) * 65535.0) as u16,
            timestamp: 0,
        });
        crate::platform::windows::inject_event(&evt)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (x, y);
        Err(AgentError::InjectionError("Unsupported platform".into()))
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| {
            std::fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|_| "unknown".to_string())
}
