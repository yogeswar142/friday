import React, { useState } from "react";
import {
  Check,
  Globe,
  Monitor,
  MousePointer,
  Power,
  Save,
  Sliders,
  SunMoon,
} from "lucide-react";
import { SettingsDto } from "../types";

interface SettingsProps {
  settings: SettingsDto;
  onSaveSettings: (settings: SettingsDto) => void;
}

export const Settings: React.FC<SettingsProps> = ({ settings, onSaveSettings }) => {
  const [form, setForm] = useState<SettingsDto>(settings);
  const [savedNotice, setSavedNotice] = useState(false);

  const handleChange = <K extends keyof SettingsDto>(key: K, value: SettingsDto[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = () => {
    onSaveSettings(form);
    setSavedNotice(true);
    setTimeout(() => setSavedNotice(false), 2500);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* Save Button Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          background: "var(--bg-card)",
          padding: "16px 20px",
          borderRadius: "var(--radius-md)",
          border: "1px solid var(--border-subtle)",
        }}
      >
        <div>
          <h2 style={{ fontSize: "16px", fontWeight: 700, display: "flex", alignItems: "center", gap: "8px" }}>
            <Sliders size={18} color="#06b6d4" />
            Configuration & Behavioral Settings
          </h2>
          <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
            Settings are stored persistently across Linux, Windows, and macOS user config directories
          </p>
        </div>

        <button className="btn btn-primary" onClick={handleSave}>
          {savedNotice ? <Check size={14} /> : <Save size={14} />}
          <span>{savedNotice ? "Saved to Disk!" : "Save Changes"}</span>
        </button>
      </div>

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
                Minimum time the cursor must dwell on screen boundary before ownership transfers (prevents accidental jumps)
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

        {/* Network & Transport Settings */}
        <div className="card" style={{ gap: "16px" }}>
          <h3 className="card-title">
            <Globe size={16} color="#3b82f6" />
            Network & Discovery
          </h3>

          <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: "13px" }}>UDP Broadcast Discovery</div>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Automatically detect and announce FRIDAY instances on local network
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
                Default Listening Port
              </label>
              <input
                type="number"
                value={form.peer_port}
                onChange={(e) => handleChange("peer_port", parseInt(e.target.value, 10))}
                style={{ width: "100%" }}
              />
              <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                Default UDP port used for input datagram transport (default: 48700)
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
                Automatic local control restoration timeout if remote peer stops acknowledging packets
              </span>
            </div>
          </div>
        </div>

        {/* Startup & System Integration */}
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
        </div>

        {/* Appearance & Advanced */}
        <div className="card" style={{ gap: "16px" }}>
          <h3 className="card-title">
            <SunMoon size={16} color="#f59e0b" />
            Appearance & Advanced
          </h3>

          <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
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

            <div>
              <label style={{ display: "block", fontWeight: 600, fontSize: "13px", marginBottom: "4px" }}>
                Log Verbosity Level
              </label>
              <select
                value={form.log_level}
                onChange={(e) => handleChange("log_level", e.target.value)}
                style={{ width: "100%" }}
              >
                <option value="error">Error</option>
                <option value="warn">Warn</option>
                <option value="info">Info (Default)</option>
                <option value="debug">Debug</option>
                <option value="trace">Trace (Verbose input telemetry)</option>
              </select>
            </div>

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: "13px" }}>Developer Inspection Mode</div>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Expose raw bincode datagrams and low-level socket metrics
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
    </div>
  );
};
