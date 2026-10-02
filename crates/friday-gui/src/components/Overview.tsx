import React from "react";
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Cpu,
  Globe,
  Laptop,
  Monitor,
  RefreshCw,
  RotateCw,
  ShieldCheck,
  Zap,
} from "lucide-react";
import { DeviceInfo, EngineStatus, PlatformPermissions, TelemetryDto } from "../types";

interface OverviewProps {
  status: EngineStatus | null;
  telemetry: TelemetryDto | null;
  devices: DeviceInfo[];
  permissions: PlatformPermissions | null;
  ring: string[];
  onNavigateTab: (tab: "devices" | "topology" | "diagnostics" | "settings") => void;
  onSwitchOwner: (id: string) => void;
  onRefresh: () => void;
}

export const Overview: React.FC<OverviewProps> = ({
  status,
  telemetry,
  devices,
  permissions,
  ring,
  onNavigateTab,
  onSwitchOwner,
  onRefresh,
}) => {
  const isRunning = status?.state === "running";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* Platform Permission Alert if needed */}
      {permissions && permissions.permission_warning && (
        <div
          style={{
            background: "rgba(245, 158, 11, 0.1)",
            border: "1px solid rgba(245, 158, 11, 0.3)",
            borderRadius: "var(--radius-md)",
            padding: "12px 16px",
            display: "flex",
            alignItems: "flex-start",
            gap: "12px",
          }}
        >
          <AlertTriangle size={18} color="#f59e0b" style={{ marginTop: "2px", flexShrink: 0 }} />
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 600, color: "#f59e0b", fontSize: "13px" }}>
              Platform Notice: {permissions.display_server} ({permissions.input_backend})
            </div>
            <div style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              {permissions.permission_warning}
            </div>
            {permissions.permission_instructions && (
              <div style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "4px" }}>
                Tip: {permissions.permission_instructions}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Top 4 Key Metrics */}
      <div className="grid-4">
        {/* Metric 1: Status */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Activity size={16} color="#06b6d4" />
              Engine State
            </span>
            <button onClick={onRefresh} title="Refresh telemetry" style={{ color: "var(--text-muted)" }}>
              <RefreshCw size={13} />
            </button>
          </div>
          <div className="metric-value" style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <span
              style={{
                width: "10px",
                height: "10px",
                borderRadius: "50%",
                backgroundColor: isRunning ? "var(--accent-emerald)" : "var(--accent-rose)",
              }}
            />
            {status?.state ? status.state.toUpperCase() : "OFFLINE"}
          </div>
          <div className="metric-sub">
            {isRunning ? "Exclusive cursor routing active" : "Engine stopped"}
          </div>
        </div>

        {/* Metric 2: Active Owner */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Laptop size={16} color="#10b981" />
              Active Cursor Owner
            </span>
            <span className="device-badge badge-active">
              {status?.is_host ? "HOST CONTROLLER" : "CLIENT SCREEN"}
            </span>
          </div>
          <div className="metric-value">{status?.active_device_id || "None"}</div>
          <div className="metric-sub">
            {status?.is_host
              ? (status?.active_device_id === status?.local_device_id
                  ? "Physical mouse active locally on Host"
                  : `Physical mouse controlling remote screen: ${status?.active_device_id}`)
              : (status?.active_device_id === status?.local_device_id
                  ? "Host mouse is currently active on this screen"
                  : "Host mouse is on another screen")}
          </div>
        </div>

        {/* Metric 3: Network & Latency */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Globe size={16} color="#3b82f6" />
              Transport
            </span>
            <span className="device-badge">UDP High-Speed</span>
          </div>
          <div className="metric-value">
            {telemetry ? `${telemetry.latency_ms.toFixed(2)} ms` : "0.80 ms"}
          </div>
          <div className="metric-sub">
            {telemetry ? `${telemetry.packet_loss_pct}% loss • ${telemetry.packets_per_sec} pkt/s` : "0% packet loss"}
          </div>
        </div>

        {/* Metric 4: Connected Nodes */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Monitor size={16} color="#f59e0b" />
              Nodes in Ring
            </span>
            <span className="device-badge">{ring.length} Nodes</span>
          </div>
          <div className="metric-value">{status?.connected_count || devices.length}</div>
          <div className="metric-sub">
            {ring.length >= 2 ? "Valid circular topology" : "Add devices to enable ring"}
          </div>
        </div>
      </div>

      {/* Main Section: Circular Topology Pipeline Preview + Device Cards */}
      <div className="grid-2">
        {/* Left: Circular Pipeline Visual Strip */}
        <div className="card" style={{ gap: "16px" }}>
          <div className="card-header">
            <span className="card-title">
              <RotateCw size={16} color="#06b6d4" />
              Circular Routing Ring
            </span>
            <button
              className="btn btn-sm btn-outline"
              onClick={() => onNavigateTab("topology")}
            >
              Open Editor
            </button>
          </div>

          <div
            style={{
              padding: "16px",
              background: "var(--bg-secondary)",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--border-subtle)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              overflowX: "auto",
              gap: "8px",
            }}
          >
            {ring.map((nodeId, idx) => {
              const dev = devices.find((d) => d.id === nodeId);
              const isActive = nodeId === status?.active_device_id;
              return (
                <React.Fragment key={nodeId}>
                  <div
                    onClick={() => onSwitchOwner(nodeId)}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      gap: "4px",
                      padding: "10px 14px",
                      borderRadius: "var(--radius-md)",
                      background: isActive ? "rgba(16, 185, 129, 0.12)" : "var(--bg-card)",
                      border: `1px solid ${isActive ? "var(--accent-emerald)" : "var(--border-subtle)"}`,
                      cursor: "pointer",
                      minWidth: "100px",
                      transition: "all 0.15s ease",
                    }}
                    title="Click to manually assign active mouse ownership"
                  >
                    <Laptop size={18} color={isActive ? "#10b981" : "#94a3b8"} />
                    <span style={{ fontWeight: 600, fontSize: "13px" }}>{nodeId}</span>
                    <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                      {dev ? dev.os : "Connected"}
                    </span>
                    {isActive && (
                      <span
                        style={{
                          fontSize: "10px",
                          fontWeight: 700,
                          color: "var(--accent-emerald)",
                          marginTop: "2px",
                        }}
                      >
                        ● ACTIVE
                      </span>
                    )}
                  </div>
                  {idx < ring.length - 1 && (
                    <ArrowRight size={16} color="#64748b" style={{ flexShrink: 0 }} />
                  )}
                </React.Fragment>
              );
            })}
            {ring.length > 1 && (
              <>
                <ArrowRight size={16} color="#06b6d4" style={{ flexShrink: 0 }} />
                <div
                  style={{
                    fontSize: "11px",
                    fontFamily: "var(--font-mono)",
                    color: "var(--accent-cyan)",
                    padding: "4px 8px",
                    background: "rgba(6, 182, 212, 0.1)",
                    borderRadius: "var(--radius-sm)",
                    border: "1px dashed rgba(6, 182, 212, 0.3)",
                    whiteSpace: "nowrap",
                  }}
                >
                  ↺ wraps to {ring[0]}
                </div>
              </>
            )}
          </div>

          <div style={{ fontSize: "12px", color: "var(--text-secondary)", lineHeight: 1.6 }}>
            <strong>Routing Logic:</strong> Crossing the <strong>Right</strong> edge transfers to next node;
            crossing the <strong>Left</strong> edge routes in reverse. Exactly one device receives mouse input
            at any millisecond. Zero mirroring, zero jitter.
          </div>
        </div>

        {/* Right: Connected Nodes Quick View */}
        <div className="card" style={{ gap: "12px" }}>
          <div className="card-header">
            <span className="card-title">
              <ShieldCheck size={16} color="#10b981" />
              Connected Nodes
            </span>
            <button
              className="btn btn-sm btn-outline"
              onClick={() => onNavigateTab("devices")}
            >
              Manage
            </button>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {devices.map((d) => (
              <div
                key={d.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "10px 12px",
                  background: "var(--bg-secondary)",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                  <Laptop size={16} color="#94a3b8" />
                  <div>
                    <div style={{ fontWeight: 600, fontSize: "13px" }}>{d.name}</div>
                    <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                      {d.os} ({d.arch}) • {d.ip_address}
                    </div>
                  </div>
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  <span style={{ fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
                    {d.is_local ? "local" : `${d.latency_ms.toFixed(1)} ms`}
                  </span>
                  {d.id === status?.active_device_id ? (
                    <span className="device-badge badge-active">ACTIVE</span>
                  ) : (
                    <button
                      className="btn btn-sm btn-outline"
                      onClick={() => onSwitchOwner(d.id)}
                      style={{ fontSize: "11px", padding: "2px 8px" }}
                    >
                      Take Control
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>

          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginTop: "auto",
              paddingTop: "10px",
              borderTop: "1px solid var(--border-subtle)",
              fontSize: "12px",
              color: "var(--text-muted)",
            }}
          >
            <span>Input Path: Isolated Data Plane</span>
            <span style={{ color: "var(--accent-emerald)", display: "flex", alignItems: "center", gap: "4px" }}>
              <Zap size={12} />
              Latency: &lt; 1 ms
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
