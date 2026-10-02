import React from "react";
import {
  Activity,
  Layers,
  Monitor,
  Settings as SettingsIcon,
  Play,
  Square,
  Pause,
  RotateCw,
  Sun,
  Moon,
  Laptop,
  Mouse,
} from "lucide-react";
import { EngineStatus, TabType } from "../types";

interface NavbarProps {
  currentTab: TabType;
  onTabChange: (tab: TabType) => void;
  status: EngineStatus | null;
  onToggleEngine: () => void;
  onPauseEngine: () => void;
  theme: "dark" | "light" | "system";
  onToggleTheme: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  currentTab,
  onTabChange,
  status,
  onToggleEngine,
  onPauseEngine,
  theme,
  onToggleTheme,
}) => {
  const isRunning = status?.state === "running";
  const isPaused = status?.state === "paused";

  return (
    <header className="top-bar">
      <div className="brand-section">
        <div className="brand-logo">
          <RotateCw size={18} color="#06b6d4" />
          <span>FRIDAY</span>
        </div>
        <span className="brand-tag">CIRCULAR ROUTER</span>
      </div>

      <nav className="nav-tabs" aria-label="Main Navigation">
        <button
          className={`nav-tab ${currentTab === "overview" ? "active" : ""}`}
          onClick={() => onTabChange("overview")}
          aria-current={currentTab === "overview" ? "page" : undefined}
        >
          <Activity size={15} />
          <span>Overview</span>
        </button>

        <button
          className={`nav-tab ${currentTab === "devices" ? "active" : ""}`}
          onClick={() => onTabChange("devices")}
          aria-current={currentTab === "devices" ? "page" : undefined}
        >
          <Monitor size={15} />
          <span>Devices</span>
          {status && status.connected_count > 0 && (
            <span style={{ fontSize: "11px", opacity: 0.8 }}>({status.connected_count})</span>
          )}
        </button>

        <button
          className={`nav-tab ${currentTab === "topology" ? "active" : ""}`}
          onClick={() => onTabChange("topology")}
          aria-current={currentTab === "topology" ? "page" : undefined}
        >
          <RotateCw size={15} />
          <span>Topology</span>
        </button>

        <button
          className={`nav-tab ${currentTab === "diagnostics" ? "active" : ""}`}
          onClick={() => onTabChange("diagnostics")}
          aria-current={currentTab === "diagnostics" ? "page" : undefined}
        >
          <Layers size={15} />
          <span>Diagnostics</span>
        </button>

        <button
          className={`nav-tab ${currentTab === "settings" ? "active" : ""}`}
          onClick={() => onTabChange("settings")}
          aria-current={currentTab === "settings" ? "page" : undefined}
        >
          <SettingsIcon size={15} />
          <span>Settings</span>
        </button>
      </nav>

      <div className="quick-status">
        {status && (
          <>
            <div
              className={`status-badge ${status.state}`}
              title={`Engine state: ${status.state}`}
            >
              <span className="status-indicator-dot" />
              <span>{status.state.toUpperCase()}</span>
            </div>

            <div
              style={{
                display: "flex",
                gap: "8px",
                alignItems: "center",
                fontSize: "11px",
                fontFamily: "var(--font-mono)",
                background: "var(--bg-secondary)",
                padding: "3px 10px",
                borderRadius: "var(--radius-sm)",
                border: "1px solid var(--border-subtle)",
              }}
              title="Physical Mouse Source & Active Cursor Owner"
            >
              <span style={{ color: "var(--text-muted)", display: "flex", alignItems: "center", gap: "4px" }}>
                <Mouse size={12} color="#06b6d4" />
                <span>PHYSICAL MOUSE:</span>
                <strong style={{ color: "var(--text-primary)" }}>{status.local_device_id}</strong>
              </span>
              <span style={{ color: "var(--border-subtle)" }}>|</span>
              <span style={{ color: "var(--accent-emerald)", display: "flex", alignItems: "center", gap: "4px" }}>
                <Laptop size={12} color="#10b981" />
                <span>ACTIVE CURSOR:</span>
                <strong>{status.active_device_id}</strong>
              </span>
            </div>
          </>
        )}

        <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
          <button
            className={`btn btn-sm ${isRunning ? "btn-danger" : "btn-success"}`}
            onClick={onToggleEngine}
            title={isRunning ? "Stop FRIDAY Engine" : "Start FRIDAY Engine"}
          >
            {isRunning ? <Square size={12} /> : <Play size={12} />}
            <span>{isRunning ? "Stop" : "Start"}</span>
          </button>

          {isRunning && (
            <button
              className={`btn btn-sm btn-outline`}
              onClick={onPauseEngine}
              title={isPaused ? "Resume Handover" : "Pause Handover"}
            >
              <Pause size={12} />
              <span>{isPaused ? "Resume" : "Pause"}</span>
            </button>
          )}

          <button
            className="btn btn-sm btn-outline"
            onClick={onToggleTheme}
            title="Toggle Theme"
            style={{ padding: "6px" }}
          >
            {theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}
          </button>
        </div>
      </div>
    </header>
  );
};
