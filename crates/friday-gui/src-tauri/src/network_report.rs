use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacketBreakdown {
    pub mouse_moves: u64,
    pub mouse_buttons: u64,
    pub keyboard_events: u64,
    pub control_packets: u64,
    pub clipboard_packets: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEventDto {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkWorkingReportDto {
    pub session_duration: String,
    pub session_start_time: String,
    pub local_role: String,
    pub local_device_name: String,
    pub local_device_id: String,
    pub local_ip: String,
    pub local_port: u16,
    pub active_device_name: String,
    pub active_device_id: String,
    pub is_controlling_remote: bool,
    pub connected_peers_count: usize,
    pub peers_summary: Vec<String>,
    pub total_tx_packets: u64,
    pub total_tx_bytes: u64,
    pub total_rx_packets: u64,
    pub total_rx_bytes: u64,
    pub current_tx_pps: u32,
    pub current_rx_pps: u32,
    pub current_tx_kbps: f32,
    pub current_rx_kbps: f32,
    pub tx_breakdown: NetworkPacketBreakdown,
    pub rx_breakdown: NetworkPacketBreakdown,
    pub latency_ms: f32,
    pub min_latency_ms: f32,
    pub max_latency_ms: f32,
    pub avg_latency_ms: f32,
    pub jitter_ms: f32,
    pub max_jitter_ms: f32,
    pub stall_count: u64,
    pub last_stall_ms: u64,
    pub max_stall_ms: u64,
    pub network_health: String,
    pub timeline: Vec<TimelineEventDto>,
    pub formatted_report: String,
}

#[derive(Debug, Clone)]
pub struct ReportContext<'a> {
    pub local_role: &'a str,
    pub local_device_name: &'a str,
    pub local_device_id: &'a str,
    pub local_ip: &'a str,
    pub local_port: u16,
    pub active_device_name: &'a str,
    pub active_device_id: &'a str,
    pub is_controlling_remote: bool,
    pub peers: &'a [crate::types::DeviceInfo],
}

/// Global Lock-Free Telemetry Hub
pub struct NetworkTelemetryHub {
    pub start_time_epoch_s: AtomicU64,
    pub total_tx_packets: AtomicU64,
    pub total_tx_bytes: AtomicU64,
    pub total_rx_packets: AtomicU64,
    pub total_rx_bytes: AtomicU64,

    pub tx_mouse_moves: AtomicU64,
    pub tx_mouse_buttons: AtomicU64,
    pub tx_keyboard: AtomicU64,
    pub tx_control: AtomicU64,
    pub tx_clipboard: AtomicU64,

    pub rx_mouse_moves: AtomicU64,
    pub rx_mouse_buttons: AtomicU64,
    pub rx_keyboard: AtomicU64,
    pub rx_control: AtomicU64,
    pub rx_clipboard: AtomicU64,

    pub last_tx_time_us: AtomicU64,
    pub last_rx_time_us: AtomicU64,
    pub last_rx_gap_us: AtomicU64,

    pub rtt_us: AtomicU64,
    pub min_rtt_us: AtomicU64,
    pub max_rtt_us: AtomicU64,
    pub avg_rtt_us: AtomicU64,

    pub jitter_us: AtomicU64,
    pub max_jitter_us: AtomicU64,

    pub stall_count: AtomicU64,
    pub last_stall_ms: AtomicU64,
    pub max_stall_ms: AtomicU64,

    pub window_tx_packets: AtomicU32,
    pub window_tx_bytes: AtomicU32,
    pub window_rx_packets: AtomicU32,
    pub window_rx_bytes: AtomicU32,

    pub current_tx_pps: AtomicU32,
    pub current_rx_pps: AtomicU32,
    pub current_tx_kbps: AtomicU32,
    pub current_rx_kbps: AtomicU32,

    timeline: Mutex<VecDeque<TimelineEventDto>>,
}

impl Default for NetworkTelemetryHub {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkTelemetryHub {
    pub fn new() -> Self {
        let epoch_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            start_time_epoch_s: AtomicU64::new(epoch_now),
            total_tx_packets: AtomicU64::new(0),
            total_tx_bytes: AtomicU64::new(0),
            total_rx_packets: AtomicU64::new(0),
            total_rx_bytes: AtomicU64::new(0),

            tx_mouse_moves: AtomicU64::new(0),
            tx_mouse_buttons: AtomicU64::new(0),
            tx_keyboard: AtomicU64::new(0),
            tx_control: AtomicU64::new(0),
            tx_clipboard: AtomicU64::new(0),

            rx_mouse_moves: AtomicU64::new(0),
            rx_mouse_buttons: AtomicU64::new(0),
            rx_keyboard: AtomicU64::new(0),
            rx_control: AtomicU64::new(0),
            rx_clipboard: AtomicU64::new(0),

            last_tx_time_us: AtomicU64::new(0),
            last_rx_time_us: AtomicU64::new(0),
            last_rx_gap_us: AtomicU64::new(0),

            rtt_us: AtomicU64::new(800),     // 0.8ms initial estimate
            min_rtt_us: AtomicU64::new(800), // 0.8ms
            max_rtt_us: AtomicU64::new(800),
            avg_rtt_us: AtomicU64::new(800),

            jitter_us: AtomicU64::new(0),
            max_jitter_us: AtomicU64::new(0),

            stall_count: AtomicU64::new(0),
            last_stall_ms: AtomicU64::new(0),
            max_stall_ms: AtomicU64::new(0),

            window_tx_packets: AtomicU32::new(0),
            window_tx_bytes: AtomicU32::new(0),
            window_rx_packets: AtomicU32::new(0),
            window_rx_bytes: AtomicU32::new(0),

            current_tx_pps: AtomicU32::new(0),
            current_rx_pps: AtomicU32::new(0),
            current_tx_kbps: AtomicU32::new(0),
            current_rx_kbps: AtomicU32::new(0),

            timeline: Mutex::new(VecDeque::with_capacity(300)),
        }
    }

    #[inline(always)]
    pub fn now_micros() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    #[inline(always)]
    pub fn record_tx(&self, bytes: usize, category: &str) {
        self.total_tx_packets.fetch_add(1, Ordering::Relaxed);
        self.total_tx_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.window_tx_packets.fetch_add(1, Ordering::Relaxed);
        self.window_tx_bytes
            .fetch_add(bytes as u32, Ordering::Relaxed);
        self.last_tx_time_us
            .store(Self::now_micros(), Ordering::Relaxed);

        match category {
            "mouse_move" => {
                self.tx_mouse_moves.fetch_add(1, Ordering::Relaxed);
            }
            "mouse_button" => {
                self.tx_mouse_buttons.fetch_add(1, Ordering::Relaxed);
            }
            "keyboard" => {
                self.tx_keyboard.fetch_add(1, Ordering::Relaxed);
            }
            "control" => {
                self.tx_control.fetch_add(1, Ordering::Relaxed);
            }
            "clipboard" => {
                self.tx_clipboard.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    #[inline(always)]
    pub fn record_rx(&self, bytes: usize, category: &str, is_actively_receiving: bool) {
        let now_us = Self::now_micros();
        self.total_rx_packets.fetch_add(1, Ordering::Relaxed);
        self.total_rx_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.window_rx_packets.fetch_add(1, Ordering::Relaxed);
        self.window_rx_bytes
            .fetch_add(bytes as u32, Ordering::Relaxed);

        let prev_rx_us = self.last_rx_time_us.swap(now_us, Ordering::Relaxed);
        if prev_rx_us > 0 {
            let gap_us = now_us.saturating_sub(prev_rx_us);
            let prev_gap = self.last_rx_gap_us.swap(gap_us, Ordering::Relaxed);

            // RFC 3550 Interarrival Jitter calculation: J = J + (|D(i-1,i)| - J)/16
            let diff_us = gap_us.abs_diff(prev_gap);
            let cur_jitter = self.jitter_us.load(Ordering::Relaxed);
            let new_jitter = (cur_jitter * 15 + diff_us) / 16;
            self.jitter_us.store(new_jitter, Ordering::Relaxed);

            let max_j = self.max_jitter_us.load(Ordering::Relaxed);
            if new_jitter > max_j {
                self.max_jitter_us.store(new_jitter, Ordering::Relaxed);
            }

            // Stall detection: If gap exceeds 120ms during continuous remote input stream,
            // log potential WiFi drop, powersave sleep, or CPU scheduler stall.
            let gap_ms = gap_us / 1000;
            if gap_ms >= 120 && is_actively_receiving {
                self.stall_count.fetch_add(1, Ordering::Relaxed);
                self.last_stall_ms.store(gap_ms, Ordering::Relaxed);
                let cur_max = self.max_stall_ms.load(Ordering::Relaxed);
                if gap_ms > cur_max {
                    self.max_stall_ms.store(gap_ms, Ordering::Relaxed);
                }
                self.add_timeline_event(
                    "WARN",
                    "network::stall",
                    &format!(
                        "Inter-packet gap of {}ms detected (potential WiFi frame drop or CPU stall)",
                        gap_ms
                    ),
                );
            }
        }

        match category {
            "mouse_move" => {
                self.rx_mouse_moves.fetch_add(1, Ordering::Relaxed);
            }
            "mouse_button" => {
                self.rx_mouse_buttons.fetch_add(1, Ordering::Relaxed);
            }
            "keyboard" => {
                self.rx_keyboard.fetch_add(1, Ordering::Relaxed);
            }
            "control" => {
                self.rx_control.fetch_add(1, Ordering::Relaxed);
            }
            "clipboard" => {
                self.rx_clipboard.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    #[inline(always)]
    pub fn record_pong(&self, send_ts_us: u32) {
        let now_32 = (Self::now_micros() & 0xFFFFFFFF) as u32;
        let rtt_us = now_32.wrapping_sub(send_ts_us) as u64;

        if rtt_us < 5_000_000 {
            // Less than 5 seconds - valid ping/pong roundtrip
            self.rtt_us.store(rtt_us, Ordering::Relaxed);

            let min = self.min_rtt_us.load(Ordering::Relaxed);
            if rtt_us < min || min == 0 {
                self.min_rtt_us.store(rtt_us, Ordering::Relaxed);
            }

            let max = self.max_rtt_us.load(Ordering::Relaxed);
            if rtt_us > max {
                self.max_rtt_us.store(rtt_us, Ordering::Relaxed);
            }

            let avg = self.avg_rtt_us.load(Ordering::Relaxed);
            let new_avg = if avg == 0 {
                rtt_us
            } else {
                (avg * 7 + rtt_us) / 8
            };
            self.avg_rtt_us.store(new_avg, Ordering::Relaxed);
        }
    }

    /// Periodic 1-second rate recalculation
    pub fn tick_rates(&self) {
        let tx_p = self.window_tx_packets.swap(0, Ordering::Relaxed);
        let tx_b = self.window_tx_bytes.swap(0, Ordering::Relaxed);
        let rx_p = self.window_rx_packets.swap(0, Ordering::Relaxed);
        let rx_b = self.window_rx_bytes.swap(0, Ordering::Relaxed);

        self.current_tx_pps.store(tx_p, Ordering::Relaxed);
        self.current_rx_pps.store(rx_p, Ordering::Relaxed);
        self.current_tx_kbps.store(tx_b / 1024, Ordering::Relaxed);
        self.current_rx_kbps.store(rx_b / 1024, Ordering::Relaxed);
    }

    /// Adds a milestone event to the bounded timeline ring buffer
    pub fn add_timeline_event(&self, level: &str, category: &str, message: &str) {
        let ts = Self::current_timestamp_string();
        if let Ok(mut timeline) = self.timeline.lock() {
            if timeline.len() >= 250 {
                timeline.pop_front();
            }
            timeline.push_back(TimelineEventDto {
                timestamp: ts,
                level: level.to_string(),
                category: category.to_string(),
                message: message.to_string(),
            });
        }
    }

    fn current_timestamp_string() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let total_secs = now.as_secs();
        let millis = now.subsec_millis();
        let hours = (total_secs / 3600) % 24;
        let mins = (total_secs / 60) % 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
    }

    pub fn get_timeline(&self) -> Vec<TimelineEventDto> {
        if let Ok(timeline) = self.timeline.lock() {
            timeline.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn build_report(&self, ctx: &ReportContext<'_>) -> NetworkWorkingReportDto {
        let epoch_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start_epoch = self.start_time_epoch_s.load(Ordering::Relaxed);
        let elapsed_secs = epoch_now.saturating_sub(start_epoch);

        let dur_hours = elapsed_secs / 3600;
        let dur_mins = (elapsed_secs % 3600) / 60;
        let dur_secs = elapsed_secs % 60;
        let session_duration = format!("{:02}:{:02}:{:02}", dur_hours, dur_mins, dur_secs);

        let start_hours = (start_epoch / 3600) % 24;
        let start_mins = (start_epoch / 60) % 60;
        let start_secs = start_epoch % 60;
        let session_start_time =
            format!("{:02}:{:02}:{:02} UTC", start_hours, start_mins, start_secs);

        let total_tx_packets = self.total_tx_packets.load(Ordering::Relaxed);
        let total_tx_bytes = self.total_tx_bytes.load(Ordering::Relaxed);
        let total_rx_packets = self.total_rx_packets.load(Ordering::Relaxed);
        let total_rx_bytes = self.total_rx_bytes.load(Ordering::Relaxed);

        let tx_breakdown = NetworkPacketBreakdown {
            mouse_moves: self.tx_mouse_moves.load(Ordering::Relaxed),
            mouse_buttons: self.tx_mouse_buttons.load(Ordering::Relaxed),
            keyboard_events: self.tx_keyboard.load(Ordering::Relaxed),
            control_packets: self.tx_control.load(Ordering::Relaxed),
            clipboard_packets: self.tx_clipboard.load(Ordering::Relaxed),
        };

        let rx_breakdown = NetworkPacketBreakdown {
            mouse_moves: self.rx_mouse_moves.load(Ordering::Relaxed),
            mouse_buttons: self.rx_mouse_buttons.load(Ordering::Relaxed),
            keyboard_events: self.rx_keyboard.load(Ordering::Relaxed),
            control_packets: self.rx_control.load(Ordering::Relaxed),
            clipboard_packets: self.rx_clipboard.load(Ordering::Relaxed),
        };

        let latency_ms = (self.rtt_us.load(Ordering::Relaxed) as f32) / 1000.0;
        let min_latency_ms = (self.min_rtt_us.load(Ordering::Relaxed) as f32) / 1000.0;
        let max_latency_ms = (self.max_rtt_us.load(Ordering::Relaxed) as f32) / 1000.0;
        let avg_latency_ms = (self.avg_rtt_us.load(Ordering::Relaxed) as f32) / 1000.0;

        let jitter_ms = (self.jitter_us.load(Ordering::Relaxed) as f32) / 1000.0;
        let max_jitter_ms = (self.max_jitter_us.load(Ordering::Relaxed) as f32) / 1000.0;

        let stall_count = self.stall_count.load(Ordering::Relaxed);
        let last_stall_ms = self.last_stall_ms.load(Ordering::Relaxed);
        let max_stall_ms = self.max_stall_ms.load(Ordering::Relaxed);

        let current_tx_pps = self.current_tx_pps.load(Ordering::Relaxed);
        let current_rx_pps = self.current_rx_pps.load(Ordering::Relaxed);
        let current_tx_kbps = self.current_tx_kbps.load(Ordering::Relaxed) as f32;
        let current_rx_kbps = self.current_rx_kbps.load(Ordering::Relaxed) as f32;

        let network_health = if stall_count == 0 && latency_ms < 5.0 && jitter_ms < 2.0 {
            "Optimal (Sub-5ms, Zero Stalls)".to_string()
        } else if stall_count <= 2 && latency_ms < 15.0 {
            "Good (Minor WiFi Jitter)".to_string()
        } else {
            format!(
                "Stalled / Congested ({} stalls detected, max {}ms)",
                stall_count, max_stall_ms
            )
        };

        let peers_summary: Vec<String> = ctx
            .peers
            .iter()
            .filter(|p| !p.is_local)
            .map(|p| {
                format!(
                    "{} ({}:{}) - {} [Active: {}]",
                    p.name, p.ip_address, p.port, p.connection_state, p.is_active
                )
            })
            .collect();

        let timeline = self.get_timeline();

        // Generate full formatted markdown text report ready for one-click copy
        let mut report_text = String::with_capacity(4096);
        report_text.push_str("=================================================================\n");
        report_text.push_str("          FRIDAY REAL-TIME NETWORK & INPUT WORKING REPORT        \n");
        report_text
            .push_str("=================================================================\n\n");

        report_text.push_str("1. SESSION OVERVIEW\n");
        report_text.push_str(&format!(
            "  - Session Start:       {}\n",
            session_start_time
        ));
        report_text.push_str(&format!("  - Elapsed Duration:    {}\n", session_duration));
        report_text.push_str(&format!(
            "  - Local Machine Role:  {} ({})\n",
            ctx.local_role, ctx.local_device_name
        ));
        report_text.push_str(&format!(
            "  - Local Endpoint:      {}:{}\n",
            ctx.local_ip, ctx.local_port
        ));
        report_text.push_str(&format!(
            "  - Local Device ID:     {}\n",
            ctx.local_device_id
        ));
        report_text.push_str(&format!(
            "  - Active Input Owner:  {} ({})\n",
            ctx.active_device_name, ctx.active_device_id
        ));
        report_text.push_str(&format!(
            "  - Controlling Remote:  {}\n",
            ctx.is_controlling_remote
        ));
        report_text.push_str(&format!(
            "  - Connected Peers:     {}\n",
            ctx.peers.len().saturating_sub(1)
        ));
        for p in &peers_summary {
            report_text.push_str(&format!("      * {}\n", p));
        }
        report_text.push('\n');

        report_text.push_str("2. NETWORK TRAFFIC & THROUGHPUT METRICS\n");
        report_text.push_str(&format!(
            "  - Total Packets Sent:      {} ({:.2} MB)\n",
            total_tx_packets,
            total_tx_bytes as f64 / 1_048_576.0
        ));
        report_text.push_str(&format!(
            "  - Total Packets Received:  {} ({:.2} MB)\n",
            total_rx_packets,
            total_rx_bytes as f64 / 1_048_576.0
        ));
        report_text.push_str(&format!(
            "  - Real-time Transmit Rate: {} pps ({:.1} KB/s)\n",
            current_tx_pps, current_tx_kbps
        ));
        report_text.push_str(&format!(
            "  - Real-time Receive Rate:  {} pps ({:.1} KB/s)\n",
            current_rx_pps, current_rx_kbps
        ));
        report_text.push('\n');

        report_text.push_str("3. INPUT EVENT TRANSMISSION BREAKDOWN\n");
        report_text.push_str("  [Transmitted to Peer (Host Mode)]:\n");
        report_text.push_str(&format!(
            "    * Mouse Motion Packets:  {}\n",
            tx_breakdown.mouse_moves
        ));
        report_text.push_str(&format!(
            "    * Mouse Button Clicks:   {}\n",
            tx_breakdown.mouse_buttons
        ));
        report_text.push_str(&format!(
            "    * Keyboard Keystrokes:   {}\n",
            tx_breakdown.keyboard_events
        ));
        report_text.push_str(&format!(
            "    * Control / Ping / Edge: {}\n",
            tx_breakdown.control_packets
        ));
        report_text.push_str(&format!(
            "    * Clipboard Sync Events: {}\n",
            tx_breakdown.clipboard_packets
        ));
        report_text.push_str("  [Received from Host (Client Mode)]:\n");
        report_text.push_str(&format!(
            "    * Mouse Motion Injected: {}\n",
            rx_breakdown.mouse_moves
        ));
        report_text.push_str(&format!(
            "    * Mouse Clicks Injected: {}\n",
            rx_breakdown.mouse_buttons
        ));
        report_text.push_str(&format!(
            "    * Keystrokes Injected:   {}\n",
            rx_breakdown.keyboard_events
        ));
        report_text.push_str(&format!(
            "    * Control Handshakes:    {}\n",
            rx_breakdown.control_packets
        ));
        report_text.push_str(&format!(
            "    * Clipboard Updates:     {}\n",
            rx_breakdown.clipboard_packets
        ));
        report_text.push('\n');

        report_text.push_str("4. LATENCY, JITTER & WIFI STABILITY DIAGNOSIS\n");
        report_text.push_str(&format!(
            "  - Network Health Rating:   {}\n",
            network_health
        ));
        report_text.push_str(&format!(
            "  - Current Round-Trip RTT:  {:.2} ms\n",
            latency_ms
        ));
        report_text.push_str(&format!(
            "  - Min / Max / Avg RTT:     {:.2} ms / {:.2} ms / {:.2} ms\n",
            min_latency_ms, max_latency_ms, avg_latency_ms
        ));
        report_text.push_str(&format!(
            "  - Inter-Arrival Jitter:    {:.2} ms (peak {:.2} ms)\n",
            jitter_ms, max_jitter_ms
        ));
        report_text.push_str(&format!("  - Network Stalls (>120ms): {}\n", stall_count));
        report_text.push_str(&format!(
            "  - Last Stall Duration:     {} ms\n",
            last_stall_ms
        ));
        report_text.push_str(&format!(
            "  - Maximum Stall Duration:  {} ms\n",
            max_stall_ms
        ));
        if stall_count > 0 {
            report_text.push_str("  [DIAGNOSIS NOTE]:\n");
            report_text.push_str(
                "    Intermittent mouse/keyboard pauses were detected. Gaps exceeding 120ms\n",
            );
            report_text.push_str("    are characteristic of 2.4GHz Wi-Fi packet drops, power-saving sleep on old NICs,\n");
            report_text.push_str("    or router congestion. For seamless 60-120fps control, connect via 5GHz Wi-Fi or Ethernet.\n");
        } else {
            report_text.push_str("  [DIAGNOSIS NOTE]:\n");
            report_text
                .push_str("    Network connection is stable with zero packet delivery stalls.\n");
        }
        report_text.push('\n');

        report_text.push_str("5. CHRONOLOGICAL OPERATIONAL EVENT TIMELINE\n");
        if timeline.is_empty() {
            report_text.push_str("  (No operational milestone events recorded yet)\n");
        } else {
            for evt in &timeline {
                report_text.push_str(&format!(
                    "  [{}] {:<5} [{:<16}] {}\n",
                    evt.timestamp, evt.level, evt.category, evt.message
                ));
            }
        }
        report_text
            .push_str("\n=================================================================\n");

        NetworkWorkingReportDto {
            session_duration,
            session_start_time,
            local_role: ctx.local_role.to_string(),
            local_device_name: ctx.local_device_name.to_string(),
            local_device_id: ctx.local_device_id.to_string(),
            local_ip: ctx.local_ip.to_string(),
            local_port: ctx.local_port,
            active_device_name: ctx.active_device_name.to_string(),
            active_device_id: ctx.active_device_id.to_string(),
            is_controlling_remote: ctx.is_controlling_remote,
            connected_peers_count: ctx.peers.len().saturating_sub(1),
            peers_summary,
            total_tx_packets,
            total_tx_bytes,
            total_rx_packets,
            total_rx_bytes,
            current_tx_pps,
            current_rx_pps,
            current_tx_kbps,
            current_rx_kbps,
            tx_breakdown,
            rx_breakdown,
            latency_ms,
            min_latency_ms,
            max_latency_ms,
            avg_latency_ms,
            jitter_ms,
            max_jitter_ms,
            stall_count,
            last_stall_ms,
            max_stall_ms,
            network_health,
            timeline,
            formatted_report: report_text,
        }
    }
}

pub static GLOBAL_NETWORK_TELEMETRY: OnceLock<NetworkTelemetryHub> = OnceLock::new();

pub fn get_telemetry_hub() -> &'static NetworkTelemetryHub {
    GLOBAL_NETWORK_TELEMETRY.get_or_init(NetworkTelemetryHub::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_telemetry_counters() {
        let hub = NetworkTelemetryHub::new();
        hub.record_tx(64, "mouse_move");
        hub.record_tx(32, "keyboard");
        hub.record_rx(64, "mouse_move", false);
        hub.record_rx(32, "keyboard", false);

        assert_eq!(hub.total_tx_packets.load(Ordering::Relaxed), 2);
        assert_eq!(hub.total_tx_bytes.load(Ordering::Relaxed), 96);
        assert_eq!(hub.tx_mouse_moves.load(Ordering::Relaxed), 1);
        assert_eq!(hub.tx_keyboard.load(Ordering::Relaxed), 1);

        assert_eq!(hub.total_rx_packets.load(Ordering::Relaxed), 2);
        assert_eq!(hub.total_rx_bytes.load(Ordering::Relaxed), 96);
        assert_eq!(hub.rx_mouse_moves.load(Ordering::Relaxed), 1);
        assert_eq!(hub.rx_keyboard.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_timeline_ring_buffer_bounded() {
        let hub = NetworkTelemetryHub::new();
        for i in 0..300 {
            hub.add_timeline_event("INFO", "test", &format!("event {}", i));
        }
        let timeline = hub.get_timeline();
        assert!(timeline.len() <= 250);
        assert!(timeline.last().unwrap().message.contains("event 299"));
    }

    #[test]
    fn test_report_generation() {
        let hub = NetworkTelemetryHub::new();
        hub.record_tx(128, "mouse_move");
        hub.add_timeline_event("INFO", "system", "FRIDAY started");

        let ctx = ReportContext {
            local_role: "Main Host",
            local_device_name: "Lenovo G50",
            local_device_id: "host-123",
            local_ip: "192.168.1.10",
            local_port: 48700,
            active_device_name: "Lenovo Yoga",
            active_device_id: "client-456",
            is_controlling_remote: true,
            peers: &[],
        };
        let report = hub.build_report(&ctx);

        assert_eq!(report.local_role, "Main Host");
        assert_eq!(report.total_tx_packets, 1);
        assert!(report
            .formatted_report
            .contains("FRIDAY REAL-TIME NETWORK & INPUT WORKING REPORT"));
        assert!(report.formatted_report.contains("Lenovo G50"));
        assert!(report.formatted_report.contains("FRIDAY started"));
    }
}
