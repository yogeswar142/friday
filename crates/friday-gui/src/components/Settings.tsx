import React, { useEffect, useState } from "react";
import {
  Activity,
  Check,
  Cpu,
  Globe,
  HardDrive,
  Laptop,
  Monitor,
  MousePointer,
  Network,
  Plus,
  Power,
  RefreshCw,
  Save,
  Server,
  Shield,
  Sliders,
  SunMoon,
  Terminal,
  Wifi,
  Zap,
} from "lucide-react";
import { LocalDeviceDto, NetworkDiagnosticsDto, SettingsDto } from "../types";
import { api } from "../api/backend";

interface SettingsProps {
  settings: SettingsDto;
  onSaveSettings: (settings: SettingsDto) => void;
  onAddManualDevice?: (ip: string, port?: number, name?: string) => Promise<void> | void;
  onInitiatePairing?: (deviceId: string, pin: string, targetIp?: string) => Promise<boolean>;
  initialSubTab?: "general" | "network" | "deviceInfo";
}

export const Settings: React.FC<SettingsProps> = ({
  settings,
  onSaveSettings,
  onAddManualDevice,
  onInitiatePairing,
  initialSubTab = "general",
}) => {
  const [subTab, setSubTab] = useState<"general" | "network" | "deviceInfo">(initialSubTab);
  const [form, setForm] = useState<SettingsDto>(settings);
  const [savedNotice, setSavedNotice] = useState(false);

  // Advanced Network Diagnostics State
  const [diagnostics, setDiagnostics] = useState<NetworkDiagnosticsDto | null>(null);
  const [isLoadingDiag, setIsLoadingDiag] = useState(false);

  // Technical Device Identity State
  const [localDevice, setLocalDevice] = useState<LocalDeviceDto | null>(null);
  const [copiedId, setCopiedId] = useState(false);

  // Direct IP Fallback Connection Form State
  const [manualIp, setManualIp] = useState("");
  const [manualName, setManualName] = useState("");
  const [manualPort, setManualPort] = useState("48700");
  const [manualStatus, setManualStatus] = useState<string | null>(null);

  // Load data when switching tabs
  useEffect(() => {
    if (subTab === "network") {
      fetchDiagnostics();
    } else if (subTab === "deviceInfo") {
      api.getLocalDevice().then(setLocalDevice).catch(console.error);
    }
  }, [subTab]);

  const fetchDiagnostics = async () => {
    setIsLoadingDiag(true);
    try {
      const data = await api.getNetworkDiagnostics();
      setDiagnostics(data);
    } catch (e) {
      console.error("Failed to load network diagnostics:", e);
    } finally {
      setIsLoadingDiag(false);
    }
  };

  const handleChange = <K extends keyof SettingsDto>(key: K, value: SettingsDto[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = () => {
    onSaveSettings(form);
    setSavedNotice(true);
    setTimeout(() => setSavedNotice(false), 2500);
  };

  const handleDirectConnect = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!manualIp.trim() || !onAddManualDevice) return;
    setManualStatus("Connecting…");
    try {
      await onAddManualDevice(
        manualIp.trim(),
        parseInt(manualPort, 10) || 48700,
        manualName.trim() || undefined
      );
      setManualStatus("✅ Device added successfully");
      setManualIp("");
      setManualName("");
      setTimeout(() => setManualStatus(null), 3000);
      fetchDiagnostics();
    } catch (err) {
      setManualStatus(`❌ Connection failed: ${err}`);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* Header & Sub-navigation */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          background: "var(--bg-card)",
          padding: "14px 20px",
          borderRadius: "var(--radius-md)",
          border: "1px solid var(--border-subtle)",
          flexWrap: "wrap",
          gap: "12px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          {/* Sub-tab pills */}
          <div
            style={{
              display: "flex",
              background: "var(--bg-secondary)",
              borderRadius: "var(--radius-sm)",
              padding: "3px",
              border: "1px solid var(--border-subtle)",
            }}
          >
            <button
              onClick={() => setSubTab("general")}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "6px 14px",
                borderRadius: "var(--radius-sm)",
                fontSize: "13px",
                fontWeight: subTab === "general" ? 600 : 500,
                background: subTab === "general" ? "var(--bg-card)" : "transparent",
                color: subTab === "general" ? "var(--text-primary)" : "var(--text-secondary)",
                boxShadow: subTab === "general" ? "0 1px 3px rgba(0,0,0,0.2)" : "none",
                transition: "all 0.15s ease",
              }}
            >
              <Sliders size={14} color={subTab === "general" ? "#06b6d4" : "currentColor"} />
              General Preferences
            </button>

            <button
              onClick={() => setSubTab("network")}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "6px 14px",
                borderRadius: "var(--radius-sm)",
                fontSize: "13px",
                fontWeight: subTab === "network" ? 600 : 500,
                background: subTab === "network" ? "var(--bg-card)" : "transparent",
                color: subTab === "network" ? "var(--text-primary)" : "var(--text-secondary)",
                boxShadow: subTab === "network" ? "0 1px 3px rgba(0,0,0,0.2)" : "none",
                transition: "all 0.15s ease",
              }}
            >
              <Network size={14} color={subTab === "network" ? "#3b82f6" : "currentColor"} />
              Advanced → Network
            </button>

            <button
              onClick={() => setSubTab("deviceInfo")}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                padding: "6px 14px",
                borderRadius: "var(--radius-sm)",
                fontSize: "13px",
                fontWeight: subTab === "deviceInfo" ? 600 : 500,
                background: subTab === "deviceInfo" ? "var(--bg-card)" : "transparent",
                color: subTab === "deviceInfo" ? "var(--text-primary)" : "var(--text-secondary)",
                boxShadow: subTab === "deviceInfo" ? "0 1px 3px rgba(0,0,0,0.2)" : "none",
                transition: "all 0.15s ease",
              }}
            >
              <Cpu size={14} color={subTab === "deviceInfo" ? "#10b981" : "currentColor"} />
              Advanced → Device Information
            </button>
          </div>
        </div>

        <button className="btn btn-primary" onClick={handleSave}>
          {savedNotice ? <Check size={14} /> : <Save size={14} />}
          <span>{savedNotice ? "Saved to Disk!" : "Save Changes"}</span>
        </button>
      </div>

      {/* ── SUB-TAB 1: GENERAL PREFERENCES ── */}
      {subTab === "general" && (
        <div className="grid-2">
          {/* Mouse Behavior Settings */}
          <div className="card" style={{ gap: "16px" }}>
            <h3 className="card-title">
              <MousePointer size={16} color="#06b6d4" />
              Mouse & Edge Routing Behavior
            </h3>

            <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
              <div>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
                  <label style={{ fontWeight: 600, fontSize: "13px" }}>Edge Dwell Time (ms)</label>
                  <span style={{ fontFamily: "var(--font-mono)", fontSize: "13px", color: "var(--accent-cyan)" }}>
                    {form.edge_dwell_ms} ms ({(form.edge_dwell_ms / 1000).toFixed(1)}s)
                  </span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="2000"
                  step="50"
                  value={form.edge_dwell_ms}
                  onChange={(e) => handleChange("edge_dwell_ms", parseInt(e.target.value, 10))}
                  style={{ width: "100%", accentColor: "var(--accent-cyan)" }}
                />
                <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Minimum time cursor dwells on boundary before transferring ownership (prevents accidental jumps)
                </span>
              </div>

              <div>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
                  <label style={{ fontWeight: 600, fontSize: "13px" }}>Edge Threshold (Pixels)</label>
                  <span style={{ fontFamily: "var(--font-mono)", fontSize: "13px", color: "var(--accent-cyan)" }}>
                    {form.edge_threshold_px} px
                  </span>
                </div>
                <input
                  type="range"
                  min="1"
                  max="20"
                  step="1"
                  value={form.edge_threshold_px}
                  onChange={(e) => handleChange("edge_threshold_px", parseInt(e.target.value, 10))}
                  style={{ width: "100%", accentColor: "var(--accent-cyan)" }}
                />
                <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Width of the virtual trigger boundary along each display edge
                </span>
              </div>

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>Cursor Coordinate Continuity</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Preserve relative perpendicular position across differing screen resolutions and DPI scaling
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.cursor_continuity}
                  onChange={(e) => handleChange("cursor_continuity", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>Cursor Position Memory</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Remember last exit position when returning control back to local machine
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.cursor_memory}
                  onChange={(e) => handleChange("cursor_memory", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>
            </div>
          </div>

          {/* System & Tray Integration */}
          <div className="card" style={{ gap: "16px" }}>
            <h3 className="card-title">
              <Power size={16} color="#10b981" />
              System & Tray Integration
            </h3>

            <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>Start with System</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Launch FRIDAY background service automatically on user login
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.start_with_system}
                  onChange={(e) => handleChange("start_with_system", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>Start Minimized to Tray</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Hide the main window on startup and sit quietly in the system tray
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.start_minimized}
                  onChange={(e) => handleChange("start_minimized", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>
            </div>

            <div style={{ marginTop: "16px", paddingTop: "16px", borderTop: "1px solid var(--border-subtle)" }}>
              <h3 className="card-title" style={{ marginBottom: "14px" }}>
                <SunMoon size={16} color="#f59e0b" />
                Appearance
              </h3>

              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div>
                  <label style={{ display: "block", fontWeight: 600, fontSize: "13px", marginBottom: "4px" }}>
                    Theme
                  </label>
                  <select
                    value={form.appearance}
                    onChange={(e) => handleChange("appearance", e.target.value as "dark" | "light" | "system")}
                    style={{ width: "100%" }}
                  >
                    <option value="dark">Dark (Recommended)</option>
                    <option value="light">Light</option>
                    <option value="system">Follow System Appearance</option>
                  </select>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── SUB-TAB 2: ADVANCED → NETWORK (Section 13) ── */}
      {subTab === "network" && (
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          {/* Section 13 Network Diagnostics Card */}
          <div className="card">
            <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <h3 className="card-title">
                  <Terminal size={17} color="#3b82f6" />
                  Network Diagnostics & Socket Details
                </h3>
                <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
                  Low-level diagnostic metrics and active transport properties for developers and network troubleshooting
                </p>
              </div>

              <button className="btn btn-sm btn-outline" onClick={fetchDiagnostics} disabled={isLoadingDiag}>
                <RefreshCw size={13} className={isLoadingDiag ? "spin" : ""} />
                <span>{isLoadingDiag ? "Querying…" : "Refresh"}</span>
              </button>
            </div>

            {diagnostics ? (
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
                  gap: "12px",
                }}
              >
                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Local IP Address</div>
                  <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--accent-cyan)" }}>
                    {diagnostics.local_ip}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Remote IP Endpoint</div>
                  <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                    {diagnostics.remote_ip || "None (Listening)"}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>UDP Port</div>
                  <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                    {diagnostics.port}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Transport Protocol</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, marginTop: "4px" }}>
                    {diagnostics.transport}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Network Interface</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, marginTop: "4px" }}>
                    {diagnostics.interface}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Connection State</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, marginTop: "4px", color: "var(--accent-emerald)" }}>
                    {diagnostics.connection_state}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Packets Transferred</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                    ↑ {diagnostics.packets_sent.toLocaleString()} &nbsp; ↓ {diagnostics.packets_received.toLocaleString()}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Roundtrip Latency (RTT)</div>
                  <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--accent-cyan)" }}>
                    {diagnostics.rtt_ms.toFixed(2)} ms
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Reconnect Attempts</div>
                  <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                    {diagnostics.reconnect_attempts}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Security Protocol</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--accent-emerald)" }}>
                    Noise_XX_25519_ChaChaPoly_BLAKE2s
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Session Authentication</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, marginTop: "4px", color: "var(--accent-emerald)" }}>
                    🔐 Authenticated (6-Digit SAS Token Verified)
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Transport Encryption</div>
                  <div style={{ fontSize: "13px", fontWeight: 600, marginTop: "4px", color: "var(--accent-emerald)" }}>
                    Active (AEAD ChaCha20-Poly1305)
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)", gridColumn: "span 2" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Discovery Engine Status</div>
                  <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--text-secondary)" }}>
                    {diagnostics.discovery_status}
                  </div>
                </div>

                <div style={{ background: "var(--bg-secondary)", padding: "12px 14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)", gridColumn: "span 2" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Internal Stable Connection ID (UUID)</div>
                  <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--text-muted)" }}>
                    {diagnostics.connection_id}
                  </div>
                </div>
              </div>
            ) : (
              <div style={{ padding: "20px", textAlign: "center", color: "var(--text-muted)" }}>
                Loading network diagnostics…
              </div>
            )}
          </div>

          {/* Graceful Fallback: Direct IP / Tailscale Manual Connect (Advanced / Manual Connection) */}
          <div className="card">
            <div className="card-header">
              <div>
                <h3 className="card-title">
                  <Globe size={16} color="#06b6d4" />
                  Advanced / Manual Connection (Direct IP & Mesh Network Fallback)
                </h3>
                <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
                  Connect directly to a specific IP address when mDNS multicast is blocked on restrictive enterprise Wi-Fi or across Tailscale/VPN mesh networks
                </p>
              </div>
            </div>

            <form onSubmit={handleDirectConnect} style={{ display: "flex", gap: "12px", flexWrap: "wrap", alignItems: "flex-end" }}>
              <div style={{ flex: "1 1 200px" }}>
                <label style={{ display: "block", fontSize: "12px", color: "var(--text-secondary)", marginBottom: "6px" }}>
                  IP Address / Hostname
                </label>
                <input
                  type="text"
                  placeholder="e.g. 192.168.1.32 or 100.94.85.40"
                  value={manualIp}
                  onChange={(e) => setManualIp(e.target.value)}
                  className="input"
                  style={{
                    width: "100%",
                    background: "var(--bg-secondary)",
                    border: "1px solid var(--border-subtle)",
                    borderRadius: "var(--radius-sm)",
                    padding: "8px 12px",
                    color: "var(--text-primary)",
                    fontFamily: "var(--font-mono)",
                    fontSize: "13px",
                  }}
                  required
                />
              </div>

              <div style={{ flex: "1 1 160px" }}>
                <label style={{ display: "block", fontSize: "12px", color: "var(--text-secondary)", marginBottom: "6px" }}>
                  Device Name (Optional)
                </label>
                <input
                  type="text"
                  placeholder="e.g. Yoga"
                  value={manualName}
                  onChange={(e) => setManualName(e.target.value)}
                  className="input"
                  style={{
                    width: "100%",
                    background: "var(--bg-secondary)",
                    border: "1px solid var(--border-subtle)",
                    borderRadius: "var(--radius-sm)",
                    padding: "8px 12px",
                    color: "var(--text-primary)",
                    fontSize: "13px",
                  }}
                />
              </div>

              <div style={{ width: "90px" }}>
                <label style={{ display: "block", fontSize: "12px", color: "var(--text-secondary)", marginBottom: "6px" }}>
                  Port
                </label>
                <input
                  type="number"
                  value={manualPort}
                  onChange={(e) => setManualPort(e.target.value)}
                  className="input"
                  style={{
                    width: "100%",
                    background: "var(--bg-secondary)",
                    border: "1px solid var(--border-subtle)",
                    borderRadius: "var(--radius-sm)",
                    padding: "8px 12px",
                    color: "var(--text-primary)",
                    fontFamily: "var(--font-mono)",
                    fontSize: "13px",
                  }}
                />
              </div>

              <button type="submit" className="btn btn-primary" style={{ height: "38px" }}>
                <Plus size={14} />
                <span>Connect Direct IP</span>
              </button>
            </form>

            {manualStatus && (
              <div style={{ marginTop: "12px", fontSize: "13px", color: manualStatus.startsWith("✅") ? "var(--accent-emerald)" : "var(--accent-rose)" }}>
                {manualStatus}
              </div>
            )}
          </div>

          {/* Network Settings Form (Ports, Discovery, Timeout) */}
          <div className="card">
            <h3 className="card-title" style={{ marginBottom: "14px" }}>
              <Sliders size={16} color="#10b981" />
              Low-Level Socket & Protocol Settings
            </h3>

            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>mDNS & UDP Discovery</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Advertise and browse _friday._udp.local. services on LAN
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.discovery_enabled}
                  onChange={(e) => handleChange("discovery_enabled", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>

              <div>
                <label style={{ display: "block", fontWeight: 600, fontSize: "13px", marginBottom: "4px" }}>
                  Default Peer Port
                </label>
                <input
                  type="number"
                  value={form.peer_port}
                  onChange={(e) => handleChange("peer_port", parseInt(e.target.value, 10))}
                  style={{ width: "100%" }}
                />
                <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Primary high-frequency UDP port (default: 48700)
                </span>
              </div>

              <div>
                <label style={{ display: "block", fontWeight: 600, fontSize: "13px", marginBottom: "4px" }}>
                  Connection Timeout (ms)
                </label>
                <input
                  type="number"
                  value={form.timeout_ms}
                  onChange={(e) => handleChange("timeout_ms", parseInt(e.target.value, 10))}
                  style={{ width: "100%" }}
                />
                <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Restores local control if remote peer disconnects (default: 3000ms)
                </span>
              </div>

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>Developer Inspection Mode</div>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                    Log raw bincode datagrams and zero-copy packet structures
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={form.developer_mode}
                  onChange={(e) => handleChange("developer_mode", e.target.checked)}
                  style={{ width: "18px", height: "18px", accentColor: "var(--border-focus)" }}
                />
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── SUB-TAB 3: ADVANCED → DEVICE INFORMATION ── */}
      {subTab === "deviceInfo" && (
        <div className="card" style={{ gap: "20px" }}>
          <div className="card-header">
            <div>
              <h3 className="card-title">
                <Cpu size={16} color="#10b981" />
                Technical Device Identity & Specifications
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
                Developer and diagnostics inspection for persistent machine identifier, OS details, and network topology role
              </p>
            </div>
          </div>

          {localDevice ? (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: "14px" }}>
              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Configured Device Name</div>
                <div style={{ fontSize: "15px", fontWeight: 700, marginTop: "4px", color: "var(--text-primary)" }}>
                  {localDevice.display_name}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Native OS Hostname</div>
                <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                  {localDevice.hostname}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Operating System</div>
                <div style={{ fontSize: "14px", fontWeight: 600, marginTop: "4px" }}>
                  {localDevice.os}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>CPU Architecture</div>
                <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px" }}>
                  {localDevice.arch}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>FRIDAY Software Version</div>
                <div style={{ fontSize: "14px", fontWeight: 600, fontFamily: "var(--font-mono)", marginTop: "4px", color: "var(--accent-cyan)" }}>
                  v{localDevice.version}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>Network Topology Role</div>
                <div style={{ fontSize: "14px", fontWeight: 600, marginTop: "4px", color: localDevice.is_host ? "var(--accent-emerald)" : "var(--accent-blue)" }}>
                  {localDevice.is_host ? "Permanent Main Host (Physical Mouse Source)" : "Connected Client Node"}
                </div>
              </div>

              <div style={{ background: "var(--bg-secondary)", padding: "14px 16px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)", gridColumn: "1 / -1" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div style={{ fontSize: "11px", color: "var(--text-muted)", textTransform: "uppercase" }}>
                    Persistent Device ID (UUIDv4 Internal Identity)
                  </div>
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText(localDevice.device_id);
                      setCopiedId(true);
                      setTimeout(() => setCopiedId(false), 2000);
                    }}
                    style={{
                      background: "none",
                      border: "none",
                      color: copiedId ? "var(--accent-emerald)" : "var(--accent-cyan)",
                      cursor: "pointer",
                      fontSize: "12px",
                      fontWeight: 600,
                    }}
                  >
                    {copiedId ? "✓ Copied to Clipboard" : "Copy UUID"}
                  </button>
                </div>
                <div style={{ fontSize: "13px", fontFamily: "var(--font-mono)", marginTop: "6px", color: "var(--text-secondary)", wordBreak: "break-all" }}>
                  {localDevice.device_id}
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "4px" }}>
                  Stored in %APPDATA%\friday\identity.json — remains invariant across IP changes, DHCP leases, and system reboots.
                </div>
              </div>
            </div>
          ) : (
            <div style={{ padding: "24px", textAlign: "center", color: "var(--text-muted)" }}>
              Loading device specifications…
            </div>
          )}
        </div>
      )}
    </div>
  );
};

