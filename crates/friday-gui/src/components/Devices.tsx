import React, { useState } from "react";
import {
  Bell,
  CheckCircle2,
  Edit2,
  Globe,
  HardDrive,
  HelpCircle,
  Laptop,
  Monitor,
  Plus,
  RefreshCw,
  Search,
  Shield,
  Trash2,
  Unplug,
  Wifi,
  XCircle,
  Zap,
} from "lucide-react";
import { DeviceInfo, DiscoveredDevice, LocalDeviceDto, PendingPairRequest } from "../types";

interface DevicesProps {
  localDevice: LocalDeviceDto | null;
  devices: DeviceInfo[];
  discovered: DiscoveredDevice[];
  pendingRequests: PendingPairRequest[];
  activeDeviceId: string;
  isHost?: boolean;
  engineRunning?: boolean;
  onInitiatePairing: (deviceId: string, pin: string, targetIp?: string) => Promise<boolean>;
  onUnpairDevice: (deviceId: string) => void;
  onConnectDevice: (deviceId: string) => void;
  onDisconnectDevice: (deviceId: string) => void;
  onSwitchOwner: (deviceId: string) => void;
  onRefreshDiscovery: () => void;
  onRespondToPairRequest: (pin: string, accept: boolean) => void;
  onUpdateLocalName: (name: string) => Promise<void>;
  onUpdateInputPreferences?: (deviceId: string, shareMouse: boolean, shareKeyboard: boolean, shareClipboard?: boolean) => Promise<void>;
  onUpdateHostInputPreferences?: (shareMouse: boolean, shareKeyboard: boolean, shareClipboard: boolean) => Promise<void>;
  onNavigateToAdvancedNetwork?: () => void;
}

export const Devices: React.FC<DevicesProps> = ({
  localDevice,
  devices,
  discovered,
  pendingRequests,
  activeDeviceId,
  isHost = true,
  engineRunning = true,
  onInitiatePairing,
  onUnpairDevice,
  onConnectDevice,
  onDisconnectDevice,
  onSwitchOwner,
  onRefreshDiscovery,
  onRespondToPairRequest,
  onUpdateLocalName,
  onUpdateInputPreferences,
  onUpdateHostInputPreferences,
  onNavigateToAdvancedNetwork,
}) => {
  const [pairingModalDev, setPairingModalDev] = useState<DiscoveredDevice | null>(null);
  const [pairCode, setPairCode] = useState("");
  const [isPairingLoading, setIsPairingLoading] = useState(false);
  const [pairingResult, setPairingResult] = useState<"accepted" | "rejected" | null>(null);
  const [pairingError, setPairingError] = useState<string | null>(null);

  // Local Device Rename Modal State
  const [isRenameModalOpen, setIsRenameModalOpen] = useState(false);
  const [newNameInput, setNewNameInput] = useState("");
  const [isSavingName, setIsSavingName] = useState(false);

  const localDeviceInfo = devices.find((d) => d.is_local) || devices[0];
  const hostShareMouse = localDeviceInfo?.share_mouse ?? true;
  const hostShareKeyboard = localDeviceInfo?.share_keyboard ?? true;
  const hostShareClipboard = localDeviceInfo?.share_clipboard ?? true;

  const startPairingFlow = (dev: DiscoveredDevice) => {
    const pin = Math.floor(100000 + Math.random() * 900000).toString();
    setPairCode(pin);
    setPairingResult(null);
    setPairingError(null);
    setPairingModalDev(dev);
  };

  const confirmPairing = async () => {
    if (!pairingModalDev) return;
    setIsPairingLoading(true);
    setPairingResult(null);
    setPairingError(null);
    try {
      const accepted = await onInitiatePairing(pairingModalDev.id, pairCode, pairingModalDev.ip_address);
      setPairingResult(accepted ? "accepted" : "rejected");
      if (accepted) {
        setTimeout(() => {
          setPairingModalDev(null);
          setPairingResult(null);
        }, 1500);
      }
    } catch (e: unknown) {
      const errMsg = typeof e === "string" ? e
        : (e as { message?: string })?.message ?? String(e);
      setPairingResult("rejected");
      setPairingError(errMsg);
    } finally {
      setIsPairingLoading(false);
    }
  };

  const openRenameModal = () => {
    setNewNameInput(localDevice?.display_name || "");
    setIsRenameModalOpen(true);
  };

  const handleSaveName = async () => {
    if (!newNameInput.trim()) return;
    setIsSavingName(true);
    try {
      await onUpdateLocalName(newNameInput.trim());
      setIsRenameModalOpen(false);
    } catch (e) {
      console.error("Failed to rename device:", e);
    } finally {
      setIsSavingName(false);
    }
  };

  const formatPinWithSpace = (code: string) => {
    if (code.length === 6) {
      return `${code.slice(0, 3)} ${code.slice(3)}`;
    }
    return code;
  };

  const renderConnectionStateBadge = (state?: string, isConnected?: boolean) => {
    const normalized = state?.toLowerCase() || (isConnected ? "connected" : "disconnected");

    switch (normalized) {
      case "connected":
        return (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "5px",
              fontSize: "12px",
              fontWeight: 600,
              padding: "3px 9px",
              borderRadius: "var(--radius-full)",
              background: "rgba(16, 185, 129, 0.12)",
              color: "var(--accent-emerald)",
              border: "1px solid rgba(16, 185, 129, 0.3)",
            }}
          >
            <span style={{ width: "6px", height: "6px", borderRadius: "50%", background: "var(--accent-emerald)" }} />
            Connected
          </span>
        );
      case "reconnecting":
        return (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "5px",
              fontSize: "12px",
              fontWeight: 600,
              padding: "3px 9px",
              borderRadius: "var(--radius-full)",
              background: "rgba(245, 158, 11, 0.12)",
              color: "var(--accent-amber)",
              border: "1px solid rgba(245, 158, 11, 0.3)",
            }}
          >
            <span style={{ width: "6px", height: "6px", borderRadius: "50%", background: "var(--accent-amber)" }} />
            Reconnecting
          </span>
        );
      case "connecting":
      case "pairing":
        return (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "5px",
              fontSize: "12px",
              fontWeight: 600,
              padding: "3px 9px",
              borderRadius: "var(--radius-full)",
              background: "rgba(6, 182, 212, 0.12)",
              color: "var(--accent-cyan)",
              border: "1px solid rgba(6, 182, 212, 0.3)",
            }}
          >
            <span style={{ width: "6px", height: "6px", borderRadius: "50%", background: "var(--accent-cyan)" }} />
            {normalized === "pairing" ? "Pairing" : "Connecting"}
          </span>
        );
      case "unreachable":
        return (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "5px",
              fontSize: "12px",
              fontWeight: 600,
              padding: "3px 9px",
              borderRadius: "var(--radius-full)",
              background: "rgba(244, 63, 94, 0.12)",
              color: "var(--accent-rose)",
              border: "1px solid rgba(244, 63, 94, 0.3)",
            }}
          >
            <span style={{ width: "6px", height: "6px", borderRadius: "50%", background: "var(--accent-rose)" }} />
            Unreachable
          </span>
        );
      default:
        return (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "5px",
              fontSize: "12px",
              fontWeight: 600,
              padding: "3px 9px",
              borderRadius: "var(--radius-full)",
              background: "rgba(148, 163, 184, 0.12)",
              color: "var(--text-secondary)",
              border: "1px solid rgba(148, 163, 184, 0.3)",
            }}
          >
            <span style={{ width: "6px", height: "6px", borderRadius: "50%", background: "#94a3b8" }} />
            Disconnected
          </span>
        );
    }
  };

  const connectedRemotes = devices.filter((d) => !d.is_local);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
      {/* ── Incoming Pair Requests Banner (shown on remote device when pairing) ── */}
      {pendingRequests.length > 0 && (
        <div
          style={{
            background: "rgba(6, 182, 212, 0.08)",
            border: "1px solid rgba(6, 182, 212, 0.4)",
            borderRadius: "var(--radius-md)",
            padding: "16px 20px",
            display: "flex",
            flexDirection: "column",
            gap: "12px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px", fontWeight: 700, fontSize: "14px", color: "#06b6d4" }}>
            <Bell size={16} />
            Incoming Pairing Request{pendingRequests.length > 1 ? "s" : ""}
          </div>
          {pendingRequests.map((req) => (
            <div
              key={req.pin}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "12px",
                background: "var(--bg-secondary)",
                borderRadius: "var(--radius-sm)",
                padding: "16px 18px",
                border: "1px solid var(--border-subtle)",
              }}
            >
              <p style={{ fontSize: "13px", color: "var(--text-secondary)" }}>
                <strong style={{ color: "var(--text-primary)" }}>{req.from_name}</strong> wants to pair with this machine.
              </p>
              <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                Confirm that the security code on their screen matches:
              </p>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "30px",
                  fontWeight: 700,
                  letterSpacing: "6px",
                  textAlign: "center",
                  padding: "12px",
                  background: "var(--bg-card)",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--border-focus)",
                  color: "#06b6d4",
                }}
              >
                {formatPinWithSpace(req.pin)}
              </div>
              <div style={{ display: "flex", gap: "10px" }}>
                <button
                  className="btn btn-sm btn-outline"
                  style={{ flex: 1, color: "var(--accent-rose)" }}
                  onClick={() => onRespondToPairRequest(req.pin, false)}
                >
                  <XCircle size={14} />
                  Reject
                </button>
                <button
                  className="btn btn-sm btn-primary"
                  style={{ flex: 2 }}
                  onClick={() => onRespondToPairRequest(req.pin, true)}
                >
                  <CheckCircle2 size={14} />
                  Accept & Pair
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* ── 1. MAIN HOST SECTION (Prominently Visualized) ── */}
      <div
        className="card"
        style={{
          background: "linear-gradient(135deg, rgba(6, 182, 212, 0.05) 0%, rgba(59, 130, 246, 0.03) 100%), var(--bg-card)",
          border: "1px solid rgba(6, 182, 212, 0.3)",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: "12px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "14px" }}>
            <div
              style={{
                width: "48px",
                height: "48px",
                borderRadius: "var(--radius-md)",
                background: "rgba(6, 182, 212, 0.15)",
                border: "1px solid rgba(6, 182, 212, 0.4)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Monitor size={26} color="#06b6d4" />
            </div>

            <div>
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <h1 style={{ fontSize: "18px", fontWeight: 700, letterSpacing: "-0.01em" }}>
                  {localDevice?.display_name || "Main Host"}
                </h1>
                <button
                  onClick={openRenameModal}
                  title="Rename this device"
                  style={{
                    color: "var(--text-muted)",
                    padding: "3px",
                    display: "flex",
                    alignItems: "center",
                    borderRadius: "var(--radius-sm)",
                    transition: "color 0.15s ease",
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.color = "var(--text-primary)")}
                  onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
                >
                  <Edit2 size={13} />
                </button>
              </div>
              <div style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
                {localDevice?.os || "Windows 11"} • {localDevice?.arch || "ARM64"} • FRIDAY v{localDevice?.version || "0.2.0"}
              </div>
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: "6px",
                fontSize: "12px",
                fontWeight: 700,
                padding: "5px 12px",
                borderRadius: "var(--radius-full)",
                background: engineRunning ? "rgba(16, 185, 129, 0.15)" : "rgba(244, 63, 94, 0.15)",
                color: engineRunning ? "var(--accent-emerald)" : "var(--accent-rose)",
                border: `1px solid ${engineRunning ? "rgba(16, 185, 129, 0.4)" : "rgba(244, 63, 94, 0.4)"}`,
              }}
            >
              <span
                style={{
                  width: "7px",
                  height: "7px",
                  borderRadius: "50%",
                  background: engineRunning ? "var(--accent-emerald)" : "var(--accent-rose)",
                }}
              />
              {engineRunning ? "MAIN HOST (RUNNING)" : "MAIN HOST (OFFLINE)"}
            </span>
          </div>
        </div>

        {/* Host Input Sharing Controls */}
        <div
          style={{
            marginTop: "14px",
            padding: "12px 14px",
            background: "var(--bg-card)",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--border-subtle)",
            display: "flex",
            flexDirection: "column",
            gap: "8px",
            fontSize: "12px",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontWeight: 600, color: "var(--text-secondary)", fontSize: "11px", letterSpacing: "0.03em", textTransform: "uppercase" }}>
              Host Sharing Controls
            </span>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              Toggle which host inputs are shared with remote devices
            </span>
          </div>

          <div style={{ display: "flex", gap: "20px", alignItems: "center", flexWrap: "wrap" }}>
            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                cursor: "pointer",
                userSelect: "none",
                color: hostShareMouse ? "var(--text-primary)" : "var(--text-muted)",
                fontWeight: hostShareMouse ? 600 : 400,
              }}
            >
              <input
                type="checkbox"
                checked={hostShareMouse}
                onChange={(e) => {
                  if (onUpdateHostInputPreferences) {
                    onUpdateHostInputPreferences(e.target.checked, hostShareKeyboard, hostShareClipboard);
                  }
                }}
                style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
              />
              <span>🖱️ Share Mouse</span>
            </label>

            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                cursor: "pointer",
                userSelect: "none",
                color: hostShareKeyboard ? "var(--text-primary)" : "var(--text-muted)",
                fontWeight: hostShareKeyboard ? 600 : 400,
              }}
            >
              <input
                type="checkbox"
                checked={hostShareKeyboard}
                onChange={(e) => {
                  if (onUpdateHostInputPreferences) {
                    onUpdateHostInputPreferences(hostShareMouse, e.target.checked, hostShareClipboard);
                  }
                }}
                style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
              />
              <span>⌨️ Share Keyboard</span>
            </label>

            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                cursor: "pointer",
                userSelect: "none",
                color: hostShareClipboard ? "var(--text-primary)" : "var(--text-muted)",
                fontWeight: hostShareClipboard ? 600 : 400,
              }}
            >
              <input
                type="checkbox"
                checked={hostShareClipboard}
                onChange={(e) => {
                  if (onUpdateHostInputPreferences) {
                    onUpdateHostInputPreferences(hostShareMouse, hostShareKeyboard, e.target.checked);
                  }
                }}
                style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
              />
              <span>📋 Share Clipboard</span>
            </label>
          </div>
        </div>

        <div
          style={{
            marginTop: "16px",
            paddingTop: "14px",
            borderTop: "1px solid var(--border-subtle)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            flexWrap: "wrap",
            gap: "10px",
            fontSize: "12px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px", color: "var(--text-secondary)" }}>
            <Shield size={14} color="#06b6d4" />
            <span>Physical Mouse Source & Controller</span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "12px" }}>
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: "4px",
                color: "var(--accent-emerald)",
                fontWeight: 600,
                fontSize: "11px",
                background: "rgba(16, 185, 129, 0.1)",
                padding: "2px 8px",
                borderRadius: "var(--radius-full)",
                border: "1px solid rgba(16, 185, 129, 0.3)",
              }}
            >
              🔐 Verified Host
            </span>
            {onNavigateToAdvancedNetwork && (
              <button
                onClick={onNavigateToAdvancedNetwork}
                style={{
                  background: "none",
                  border: "none",
                  color: "var(--accent-cyan)",
                  cursor: "pointer",
                  fontSize: "12px",
                  padding: "0",
                  textDecoration: "underline",
                }}
              >
                Device Information →
              </button>
            )}
          </div>
        </div>
      </div>

      {/* ── 2. CONNECTED DEVICES SECTION ── */}
      <div className="card">
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h2 className="card-title">
              <Laptop size={17} color="#10b981" />
              Connected Devices ({connectedRemotes.length})
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Devices linked and participating in your shared cursor space
            </p>
          </div>
        </div>

        {connectedRemotes.length === 0 ? (
          <div
            style={{
              padding: "36px 20px",
              textAlign: "center",
              color: "var(--text-muted)",
              background: "var(--bg-secondary)",
              borderRadius: "var(--radius-sm)",
              border: "1px dashed var(--border-subtle)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: "8px",
            }}
          >
            <Laptop size={28} color="#64748b" />
            <div style={{ fontWeight: 600, color: "var(--text-secondary)" }}>No other devices connected yet</div>
            <div style={{ fontSize: "12px" }}>
              FRIDAY devices detected on your Wi-Fi will appear in Discovered Devices below.
            </div>
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))", gap: "16px" }}>
            {connectedRemotes.map((d) => {
              const isActive = d.id === activeDeviceId;

              return (
                <div
                  key={d.id}
                  style={{
                    background: "var(--bg-secondary)",
                    border: `1px solid ${isActive ? "var(--accent-emerald)" : "var(--border-subtle)"}`,
                    borderRadius: "var(--radius-md)",
                    padding: "18px",
                    display: "flex",
                    flexDirection: "column",
                    gap: "14px",
                    transition: "border-color 0.2s ease, transform 0.15s ease",
                  }}
                >
                  {/* Card Header: Friendly Name, Platform, Status */}
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
                      <div
                        style={{
                          padding: "10px",
                          background: "var(--bg-card)",
                          borderRadius: "var(--radius-sm)",
                          border: "1px solid var(--border-subtle)",
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "center",
                        }}
                      >
                        <Laptop size={22} color={isActive ? "#10b981" : "#94a3b8"} />
                      </div>
                      <div>
                        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                          <span style={{ fontWeight: 700, fontSize: "15px", color: "var(--text-primary)" }}>{d.name}</span>
                          <span
                            style={{
                              fontSize: "10px",
                              fontWeight: 600,
                              padding: "1px 6px",
                              borderRadius: "var(--radius-full)",
                              background: "rgba(16, 185, 129, 0.12)",
                              color: "var(--accent-emerald)",
                              border: "1px solid rgba(16, 185, 129, 0.3)",
                              display: "inline-flex",
                              alignItems: "center",
                              gap: "3px",
                            }}
                            title="Authenticated and encrypted session (Noise Protocol)"
                          >
                            🔐 Trusted
                          </span>
                        </div>
                        <div style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "1px" }}>
                          {d.os} • {d.arch}
                        </div>
                      </div>
                    </div>

                    <div>
                      {isActive ? (
                        <span className="device-badge badge-active">ACTIVE CURSOR</span>
                      ) : (
                        renderConnectionStateBadge(d.connection_state, d.is_connected)
                      )}
                    </div>
                  </div>

                  {/* Capabilities & Latency Row (Clean & Consumer-friendly) */}
                  <div
                    style={{
                      background: "var(--bg-card)",
                      padding: "10px 14px",
                      borderRadius: "var(--radius-sm)",
                      border: "1px solid var(--border-subtle)",
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      fontSize: "12px",
                    }}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                      <span style={{ color: (d.capabilities.includes("mouse_injection") || (d.share_mouse ?? true)) ? "var(--accent-emerald)" : "var(--text-muted)", fontWeight: 500 }}>
                        Mouse {(d.capabilities.includes("mouse_injection") || (d.share_mouse ?? true)) ? "✓" : "○"}
                      </span>
                      <span style={{ color: (d.capabilities.includes("keyboard_injection") || (d.share_keyboard ?? true)) ? "var(--accent-emerald)" : "var(--text-muted)", fontWeight: 500 }}>
                        Keyboard {(d.capabilities.includes("keyboard_injection") || (d.share_keyboard ?? true)) ? "✓" : "○"}
                      </span>
                      <span style={{ color: (d.capabilities.includes("clipboard") || (d.share_clipboard ?? true)) ? "var(--accent-emerald)" : "var(--text-muted)", fontWeight: 500 }}>
                        Clipboard {(d.capabilities.includes("clipboard") || (d.share_clipboard ?? true)) ? "✓" : "○"}
                      </span>
                    </div>

                    <div style={{ display: "flex", alignItems: "center", gap: "4px", color: "var(--accent-cyan)", fontFamily: "var(--font-mono)", fontSize: "12px" }}>
                      <Zap size={12} />
                      <span>{d.is_connected ? `${d.latency_ms > 0 ? d.latency_ms.toFixed(1) : "< 1.0"} ms` : "—"}</span>
                    </div>
                  </div>

                  {/* Input Sharing Preferences (Per-device: Mouse & Keyboard) */}
                  {isHost && (
                    <div
                      style={{
                        background: "var(--bg-card)",
                        padding: "10px 14px",
                        borderRadius: "var(--radius-sm)",
                        border: "1px solid var(--border-subtle)",
                        display: "flex",
                        flexDirection: "column",
                        gap: "8px",
                        fontSize: "12px",
                      }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <span style={{ fontWeight: 600, color: "var(--text-secondary)", fontSize: "11px", letterSpacing: "0.03em", textTransform: "uppercase" }}>
                          Input Permissions
                        </span>
                        <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                          Control remote device sharing
                        </span>
                      </div>

                      <div style={{ display: "flex", gap: "16px", alignItems: "center", flexWrap: "wrap" }}>
                        <label
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "6px",
                            cursor: "pointer",
                            userSelect: "none",
                            color: (d.share_mouse ?? true) ? "var(--text-primary)" : "var(--text-muted)",
                            fontWeight: (d.share_mouse ?? true) ? 600 : 400,
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={d.share_mouse ?? true}
                            onChange={(e) => {
                              if (onUpdateInputPreferences) {
                                onUpdateInputPreferences(d.id, e.target.checked, d.share_keyboard ?? true, d.share_clipboard ?? true);
                              }
                            }}
                            style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
                          />
                          <span>Share Mouse</span>
                        </label>

                        <label
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "6px",
                            cursor: "pointer",
                            userSelect: "none",
                            color: (d.share_keyboard ?? true) ? "var(--text-primary)" : "var(--text-muted)",
                            fontWeight: (d.share_keyboard ?? true) ? 600 : 400,
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={d.share_keyboard ?? true}
                            onChange={(e) => {
                              if (onUpdateInputPreferences) {
                                onUpdateInputPreferences(d.id, d.share_mouse ?? true, e.target.checked, d.share_clipboard ?? true);
                              }
                            }}
                            style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
                          />
                          <span>Share Keyboard</span>
                        </label>

                        <label
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "6px",
                            cursor: "pointer",
                            userSelect: "none",
                            color: (d.share_clipboard ?? true) ? "var(--text-primary)" : "var(--text-muted)",
                            fontWeight: (d.share_clipboard ?? true) ? 600 : 400,
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={d.share_clipboard ?? true}
                            onChange={(e) => {
                              if (onUpdateInputPreferences) {
                                onUpdateInputPreferences(d.id, d.share_mouse ?? true, d.share_keyboard ?? true, e.target.checked);
                              }
                            }}
                            style={{ cursor: "pointer", accentColor: "var(--accent-emerald)" }}
                          />
                          <span>Share Clipboard</span>
                        </label>
                      </div>
                    </div>
                  )}

                  {/* Actions Row */}
                  <div style={{ display: "flex", gap: "8px", marginTop: "auto" }}>
                    {isHost && !isActive && (
                      <button
                        className="btn btn-sm btn-primary"
                        onClick={() => onSwitchOwner(d.id)}
                        style={{ flex: 1 }}
                        title={`Move active mouse control directly to ${d.name}`}
                      >
                        <Zap size={12} />
                        Take Control
                      </button>
                    )}

                    {d.is_connected ? (
                      <button
                        className="btn btn-sm btn-outline"
                        onClick={() => onDisconnectDevice(d.id)}
                        title="Temporarily disconnect session"
                        style={{ flex: isHost && !isActive ? "initial" : 1 }}
                      >
                        <Unplug size={13} />
                        <span>Disconnect</span>
                      </button>
                    ) : (
                      <button
                        className="btn btn-sm btn-outline"
                        onClick={() => onConnectDevice(d.id)}
                        title="Reconnect authenticated session"
                        style={{ flex: 1 }}
                      >
                        <Wifi size={13} />
                        <span>Reconnect</span>
                      </button>
                    )}

                    <button
                      className="btn btn-sm btn-outline"
                      onClick={() => onUnpairDevice(d.id)}
                      style={{ color: "var(--accent-rose)" }}
                      title="Forget device and revoke trust"
                    >
                      <Trash2 size={13} />
                      <span>Forget</span>
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* ── 3. DISCOVERED NEARBY DEVICES SECTION ── */}
      <div className="card">
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "10px" }}>
          <div>
            <h2 className="card-title">
              <Search size={16} color="#3b82f6" />
              Discovered Nearby Devices ({discovered.length})
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Zero-configuration discovery listening on local network via mDNS (_friday._udp.local.)
            </p>
          </div>

          <button className="btn btn-sm btn-outline" onClick={onRefreshDiscovery}>
            <RefreshCw size={13} />
            <span>Scan Subnet</span>
          </button>
        </div>

        {discovered.length === 0 ? (
          <div
            style={{
              padding: "36px 20px",
              textAlign: "center",
              color: "var(--text-muted)",
              background: "var(--bg-secondary)",
              borderRadius: "var(--radius-sm)",
              border: "1px dashed var(--border-subtle)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: "8px",
            }}
          >
            <Search size={24} color="#64748b" />
            <div style={{ fontWeight: 600, color: "var(--text-secondary)" }}>
              No new devices detected on local Wi-Fi
            </div>
            <div style={{ fontSize: "12px" }}>
              Make sure FRIDAY is open and running on your other computer connected to the same Wi-Fi.
            </div>
            {onNavigateToAdvancedNetwork && (
              <button
                className="btn btn-sm btn-outline"
                style={{ marginTop: "8px", fontSize: "12px" }}
                onClick={onNavigateToAdvancedNetwork}
              >
                <Globe size={13} />
                <span>Can't find device? Use Direct IP in Advanced Network Settings →</span>
              </button>
            )}
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: "12px" }}>
            {discovered.map((disc) => (
              <div
                key={disc.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "14px 18px",
                  background: "var(--bg-secondary)",
                  borderRadius: "var(--radius-md)",
                  border: "1px solid var(--border-subtle)",
                  transition: "border-color 0.15s ease",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
                  <div
                    style={{
                      padding: "8px",
                      background: "var(--bg-card)",
                      borderRadius: "var(--radius-sm)",
                      border: "1px solid var(--border-subtle)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                    }}
                  >
                    <Laptop size={20} color="#06b6d4" />
                  </div>
                  <div>
                    <div style={{ fontWeight: 700, fontSize: "14px", color: "var(--text-primary)" }}>{disc.name}</div>
                    <div style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
                      {disc.os} • {disc.arch}
                    </div>
                    <div style={{ display: "flex", alignItems: "center", gap: "4px", fontSize: "11px", color: "var(--accent-emerald)", marginTop: "3px" }}>
                      <span style={{ width: "5px", height: "5px", borderRadius: "50%", background: "var(--accent-emerald)" }} />
                      <span>Nearby</span>
                    </div>
                  </div>
                </div>

                <button
                  className="btn btn-sm btn-primary"
                  onClick={() => startPairingFlow(disc)}
                  style={{ padding: "8px 16px", fontWeight: 600 }}
                >
                  <Shield size={13} />
                  <span>Pair</span>
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* ── 4. APPLE CONTINUITY-STYLE PAIRING MODAL ── */}
      {pairingModalDev && (
        <div className="modal-overlay" onClick={() => setPairingModalDev(null)}>
          <div
            className="modal-content"
            onClick={(e) => e.stopPropagation()}
            style={{ maxWidth: "420px", textAlign: "center", padding: "28px 24px" }}
          >
            <div
              style={{
                width: "56px",
                height: "56px",
                borderRadius: "50%",
                background: "rgba(6, 182, 212, 0.12)",
                border: "1px solid rgba(6, 182, 212, 0.3)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                margin: "0 auto 16px auto",
              }}
            >
              <Shield size={28} color="#06b6d4" />
            </div>

            <h3 style={{ fontSize: "18px", fontWeight: 700, marginBottom: "8px" }}>
              Pair with {pairingModalDev.name}
            </h3>

            <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "20px", lineHeight: 1.5 }}>
              FRIDAY wants to connect to: <strong>{pairingModalDev.name}</strong>
              <br />
              Confirm this security code on the other device:
            </p>

            {/* Human-verifiable 6-digit Code (e.g. 739 421) */}
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: "36px",
                fontWeight: 700,
                letterSpacing: "6px",
                textAlign: "center",
                padding: "16px 20px",
                background: "var(--bg-secondary)",
                borderRadius: "var(--radius-md)",
                border: `1px solid ${
                  pairingResult === "accepted"
                    ? "var(--accent-emerald)"
                    : pairingResult === "rejected"
                    ? "var(--accent-rose)"
                    : "var(--border-focus)"
                }`,
                color:
                  pairingResult === "accepted"
                    ? "#10b981"
                    : pairingResult === "rejected"
                    ? "#f43f5e"
                    : "var(--accent-cyan)",
                marginBottom: "20px",
                transition: "border-color 0.3s, color 0.3s",
              }}
            >
              {formatPinWithSpace(pairCode)}
            </div>

            {/* Status Messages */}
            {!isPairingLoading && !pairingResult && (
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginBottom: "20px" }}>
                Click <strong>Pair</strong> to transmit the pairing request to {pairingModalDev.name}.
              </p>
            )}

            {isPairingLoading && (
              <div style={{ marginBottom: "20px" }}>
                <p style={{ fontSize: "13px", color: "#06b6d4", fontWeight: 500 }}>
                  ⏳ Waiting for confirmation on {pairingModalDev.name}…
                </p>
                <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "4px" }}>
                  Please click Accept on the other machine
                </p>
              </div>
            )}

            {pairingResult === "accepted" && (
              <div style={{ marginBottom: "20px" }}>
                <p style={{ fontSize: "14px", color: "#10b981", fontWeight: 700 }}>
                  ✅ Successfully Paired!
                </p>
                <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "4px" }}>
                  {pairingModalDev.name} added to your trusted devices.
                </p>
              </div>
            )}

            {pairingResult === "rejected" && (
              <div
                style={{
                  marginBottom: "20px",
                  background: "rgba(244, 63, 94, 0.08)",
                  border: "1px solid rgba(244, 63, 94, 0.25)",
                  borderRadius: "var(--radius-sm)",
                  padding: "12px",
                  textAlign: "left",
                }}
              >
                <p style={{ fontSize: "13px", color: "#f43f5e", fontWeight: 600, marginBottom: "4px" }}>
                  ❌ {pairingError ? "Connection Failed" : "Pairing Declined"}
                </p>
                <p style={{ fontSize: "12px", color: "var(--text-muted)", margin: 0 }}>
                  {pairingError || "The remote user rejected the request or the connection timed out."}
                </p>
              </div>
            )}

            {/* Actions */}
            <div style={{ display: "flex", gap: "10px" }}>
              <button
                className="btn btn-outline"
                onClick={() => {
                  setPairingModalDev(null);
                  setPairingResult(null);
                  setPairingError(null);
                }}
                style={{ flex: 1 }}
                disabled={isPairingLoading}
              >
                {pairingResult === "accepted" ? "Done" : "Cancel"}
              </button>

              {!pairingResult && (
                <button
                  className="btn btn-primary"
                  onClick={confirmPairing}
                  disabled={isPairingLoading}
                  style={{ flex: 1.5, fontWeight: 600 }}
                >
                  {isPairingLoading ? "Connecting…" : "Pair"}
                </button>
              )}

              {pairingResult === "rejected" && (
                <button
                  className="btn btn-primary"
                  onClick={() => {
                    setPairingResult(null);
                    setPairingError(null);
                    confirmPairing();
                  }}
                  style={{ flex: 1.5 }}
                >
                  Retry
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── 5. RENAME LOCAL MACHINE MODAL ── */}
      {isRenameModalOpen && (
        <div className="modal-overlay" onClick={() => setIsRenameModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()} style={{ maxWidth: "380px" }}>
            <h3 style={{ fontSize: "16px", fontWeight: 700, marginBottom: "8px" }}>
              Rename This Device
            </h3>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginBottom: "16px" }}>
              Choose a friendly name (e.g. "Yoga", "G50", "MacBook Pro") for how this device appears to others on your network.
            </p>

            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleSaveName();
              }}
            >
              <input
                type="text"
                value={newNameInput}
                onChange={(e) => setNewNameInput(e.target.value)}
                placeholder="e.g. Yoga"
                className="input"
                style={{
                  width: "100%",
                  marginBottom: "16px",
                  fontSize: "14px",
                  padding: "10px 12px",
                }}
                autoFocus
                maxLength={40}
              />

              <div style={{ display: "flex", gap: "10px", justifyContent: "flex-end" }}>
                <button
                  type="button"
                  className="btn btn-outline"
                  onClick={() => setIsRenameModalOpen(false)}
                  disabled={isSavingName}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={!newNameInput.trim() || isSavingName}
                >
                  {isSavingName ? "Saving…" : "Save Name"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
