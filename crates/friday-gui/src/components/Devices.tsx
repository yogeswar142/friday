import React, { useState } from "react";
import {
  Bell,
  CheckCircle2,
  HardDrive,
  Laptop,
  Plus,
  RefreshCw,
  Search,
  Shield,
  Unplug,
  XCircle,
  Zap,
} from "lucide-react";
import { DeviceInfo, DiscoveredDevice, PendingPairRequest } from "../types";

interface DevicesProps {
  devices: DeviceInfo[];
  discovered: DiscoveredDevice[];
  pendingRequests: PendingPairRequest[];
  activeDeviceId: string;
  isHost?: boolean;
  onInitiatePairing: (deviceId: string, pin: string, targetIp?: string) => Promise<boolean>;
  onAddManualDevice: (ip: string, port?: number, name?: string) => void;
  onUnpairDevice: (deviceId: string) => void;
  onConnectDevice: (deviceId: string) => void;
  onDisconnectDevice: (deviceId: string) => void;
  onSwitchOwner: (deviceId: string) => void;
  onRefreshDiscovery: () => void;
  onRespondToPairRequest: (pin: string, accept: boolean) => void;
}

export const Devices: React.FC<DevicesProps> = ({
  devices,
  discovered,
  pendingRequests,
  activeDeviceId,
  isHost = true,
  onInitiatePairing,
  onAddManualDevice,
  onUnpairDevice,
  onConnectDevice,
  onDisconnectDevice,
  onSwitchOwner,
  onRefreshDiscovery,
  onRespondToPairRequest,
}) => {
  const [pairingModalDev, setPairingModalDev] = useState<DiscoveredDevice | null>(null);
  const [pairCode, setPairCode] = useState("");
  const [isPairingLoading, setIsPairingLoading] = useState(false);
  const [pairingResult, setPairingResult] = useState<"accepted" | "rejected" | null>(null);
  const [pairingError, setPairingError] = useState<string | null>(null);
  const [manualIp, setManualIp] = useState("");
  const [manualName, setManualName] = useState("");
  const [manualPort, setManualPort] = useState("48700");

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
      // Rust returned a real error string (e.g. firewall / unreachable)
      const errMsg = typeof e === "string" ? e
        : (e as { message?: string })?.message ?? String(e);
      setPairingResult("rejected");
      setPairingError(errMsg);
    } finally {
      setIsPairingLoading(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>

      {/* ── Incoming Pair Requests Banner (shown on remote device) ── */}
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
                gap: "10px",
                background: "var(--bg-secondary)",
                borderRadius: "var(--radius-sm)",
                padding: "14px 16px",
                border: "1px solid var(--border-subtle)",
              }}
            >
              <p style={{ fontSize: "13px", color: "var(--text-secondary)" }}>
                <strong style={{ color: "var(--text-primary)" }}>{req.from_name}</strong>
                {" "}({req.from_ip}) wants to pair with this machine.
              </p>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                Confirm that the PIN on their screen matches:
              </p>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "28px",
                  fontWeight: 700,
                  letterSpacing: "8px",
                  textAlign: "center",
                  padding: "12px",
                  background: "var(--bg-card)",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--border-focus)",
                  color: "#06b6d4",
                }}
              >
                {req.pin}
              </div>
              <div style={{ display: "flex", gap: "10px" }}>
                <button
                  className="btn btn-sm btn-outline"
                  style={{ flex: 1, color: "var(--accent-rose)" }}
                  onClick={() => onRespondToPairRequest(req.pin, false)}
                >
                  <XCircle size={13} />
                  Reject
                </button>
                <button
                  className="btn btn-sm btn-primary"
                  style={{ flex: 2 }}
                  onClick={() => onRespondToPairRequest(req.pin, true)}
                >
                  <CheckCircle2 size={13} />
                  Accept & Pair
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Connected Devices Section */}
      <div className="card">
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "10px" }}>
          <div>
            <h2 className="card-title">
              <Laptop size={17} color="#10b981" />
              Connected Devices ({devices.length})
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              {isHost
                ? "Main Host Mode — this machine's mouse & touchpad control all connected screens across the circular topology."
                : "Client Screen Mode — receiving remote mouse & trackpad input from the Main Host."}
            </p>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span
              style={{
                fontSize: "12px",
                fontWeight: 700,
                padding: "5px 12px",
                borderRadius: "var(--radius-sm)",
                background: isHost ? "rgba(6, 182, 212, 0.15)" : "rgba(168, 85, 247, 0.15)",
                color: isHost ? "#06b6d4" : "#a855f7",
                border: `1px solid ${isHost ? "rgba(6, 182, 212, 0.4)" : "rgba(168, 85, 247, 0.4)"}`,
                display: "flex",
                alignItems: "center",
                gap: "6px",
              }}
            >
              {isHost ? "ROLE: MAIN HOST (CONTROLLER)" : "ROLE: CLIENT SCREEN"}
            </span>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))", gap: "14px" }}>
          {devices.map((d) => {
            const isActive = d.id === activeDeviceId;

            return (
              <div
                key={d.id}
                style={{
                  background: "var(--bg-secondary)",
                  border: `1px solid ${isActive ? "var(--accent-emerald)" : "var(--border-subtle)"}`,
                  borderRadius: "var(--radius-md)",
                  padding: "16px",
                  display: "flex",
                  flexDirection: "column",
                  gap: "12px",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                    <div
                      style={{
                        padding: "8px",
                        background: "var(--bg-card)",
                        borderRadius: "var(--radius-sm)",
                        border: "1px solid var(--border-subtle)",
                      }}
                    >
                      <Laptop size={20} color={isActive ? "#10b981" : "#94a3b8"} />
                    </div>
                    <div>
                      <div style={{ fontWeight: 700, fontSize: "14px" }}>{d.name}</div>
                      <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                        {d.os} ({d.arch})
                      </div>
                    </div>
                  </div>

                  {isActive ? (
                    <span className="device-badge badge-active">ACTIVE CURSOR</span>
                  ) : (
                    <span
                      style={{
                        fontSize: "11px",
                        padding: "2px 8px",
                        borderRadius: "var(--radius-sm)",
                        background: d.is_connected ? "rgba(16, 185, 129, 0.1)" : "rgba(244, 63, 94, 0.1)",
                        color: d.is_connected ? "var(--accent-emerald)" : "var(--accent-rose)",
                        border: `1px solid ${d.is_connected ? "rgba(16, 185, 129, 0.3)" : "rgba(244, 63, 94, 0.3)"}`,
                      }}
                    >
                      {d.is_connected ? "CONNECTED" : "OFFLINE"}
                    </span>
                  )}
                </div>

                {/* Specs and Latency */}
                <div
                  style={{
                    display: "grid",
                    gridTemplateColumns: "repeat(2, 1fr)",
                    gap: "8px",
                    background: "var(--bg-card)",
                    padding: "8px 12px",
                    borderRadius: "var(--radius-sm)",
                    fontSize: "11px",
                    fontFamily: "var(--font-mono)",
                  }}
                >
                  <div>
                    <span style={{ color: "var(--text-muted)" }}>ENDPOINT: </span>
                    <span>{d.ip_address}:{d.port}</span>
                  </div>
                  <div>
                    <span style={{ color: "var(--text-muted)" }}>LATENCY: </span>
                    <span style={{ color: "var(--accent-cyan)" }}>
                      {d.is_local ? "0.0 ms" : `${d.latency_ms.toFixed(2)} ms`}
                    </span>
                  </div>
                  <div>
                    <span style={{ color: "var(--text-muted)" }}>SOURCE: </span>
                    <span>{d.is_local ? (isHost ? "Physical Mouse (Host)" : "Client Screen") : (isHost ? "Client Screen" : "Host Controller")}</span>
                  </div>
                  <div>
                    <span style={{ color: "var(--text-muted)" }}>CAPS: </span>
                    <span>{d.capabilities.length} features</span>
                  </div>
                </div>

                {/* Actions */}
                <div style={{ display: "flex", gap: "8px", marginTop: "auto" }}>
                  {isHost ? (
                    !isActive && (
                      <button
                        className="btn btn-sm btn-primary"
                        onClick={() => onSwitchOwner(d.id)}
                        style={{ flex: 1 }}
                        title={d.is_local ? "Return mouse directly to Host screen" : `Jump physical mouse directly to ${d.name}`}
                      >
                        <Zap size={12} />
                        Take Control
                      </button>
                    )
                  ) : (
                    <div style={{ flex: 1, fontSize: "11px", color: "var(--text-muted)", display: "flex", alignItems: "center", gap: "4px", padding: "4px 0" }}>
                      <Shield size={12} />
                      <span>{isActive ? "Host mouse is on this screen" : "Client Screen (Managed by Host)"}</span>
                    </div>
                  )}

                  {!d.is_local && (
                    <>
                      {d.is_connected ? (
                        <button
                          className="btn btn-sm btn-outline"
                          onClick={() => onDisconnectDevice(d.id)}
                          title="Disconnect network stream"
                        >
                          <Unplug size={12} />
                        </button>
                      ) : (
                        <button
                          className="btn btn-sm btn-outline"
                          onClick={() => onConnectDevice(d.id)}
                          title="Reconnect"
                        >
                          Connect
                        </button>
                      )}

                      <button
                        className="btn btn-sm btn-outline"
                        onClick={() => onUnpairDevice(d.id)}
                        style={{ color: "var(--accent-rose)" }}
                        title="Unpair and remove device"
                      >
                        Unpair
                      </button>
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Add Device by Direct IP / Tailscale */}
      <div className="card">
        <div className="card-header">
          <div>
            <h2 className="card-title">
              <Plus size={16} color="#06b6d4" />
              Add Remote Device (IP / Tailscale)
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Directly connect to another laptop across your local network or Tailscale mesh IP
            </p>
          </div>
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (manualIp.trim()) {
              onAddManualDevice(manualIp.trim(), parseInt(manualPort) || 48700, manualName.trim() || undefined);
              setManualIp("");
              setManualName("");
            }
          }}
          style={{ display: "flex", gap: "12px", flexWrap: "wrap", alignItems: "flex-end" }}
        >
          <div style={{ flex: "1 1 200px" }}>
            <label style={{ display: "block", fontSize: "12px", color: "var(--text-secondary)", marginBottom: "6px" }}>
              IP Address / Host
            </label>
            <input
              type="text"
              placeholder="e.g. 192.168.1.2 or 100.94.85.40"
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
              placeholder="e.g. Lenovo Yoga"
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

          <div style={{ display: "flex", gap: "8px" }}>
            <button type="submit" className="btn btn-outline" style={{ height: "38px" }}>
              <Plus size={14} />
              <span>Direct Connect</span>
            </button>
            <button
              type="button"
              className="btn btn-primary"
              style={{ height: "38px" }}
              disabled={!manualIp.trim()}
              onClick={() => {
                if (!manualIp.trim()) return;
                startPairingFlow({
                  id: manualIp.trim(),
                  name: manualName.trim() || `Node (${manualIp.trim()})`,
                  ip_address: manualIp.trim(),
                  port: parseInt(manualPort) || 48700,
                  os: "Remote",
                  arch: "x86_64",
                  is_paired: false,
                });
              }}
            >
              <Shield size={14} />
              <span>Pair with IP</span>
            </button>
          </div>
        </form>
      </div>

      {/* Discovered Nearby Devices Section */}
      <div className="card">
        <div className="card-header">
          <div>
            <h2 className="card-title">
              <Search size={16} color="#3b82f6" />
              Discovered Nearby Devices ({discovered.length})
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Zero-configuration discovery listening on local subnet (mDNS / UDP broadcast)
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
              padding: "32px",
              textAlign: "center",
              color: "var(--text-muted)",
              background: "var(--bg-secondary)",
              borderRadius: "var(--radius-sm)",
              border: "1px dashed var(--border-subtle)",
            }}
          >
            No new devices detected on local Wi-Fi. Ensure FRIDAY is installed and running on peer machine.
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
            {discovered.map((disc) => (
              <div
                key={disc.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "12px 16px",
                  background: "var(--bg-secondary)",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
                  <Laptop size={18} color="#06b6d4" />
                  <div>
                    <div style={{ fontWeight: 600, fontSize: "14px" }}>{disc.name}</div>
                    <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                      {disc.os} ({disc.arch}) • {disc.ip_address}:{disc.port}
                    </div>
                  </div>
                </div>

                <button
                  className="btn btn-sm btn-primary"
                  onClick={() => startPairingFlow(disc)}
                >
                  <Shield size={13} />
                  <span>Pair Node</span>
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Pairing Confirmation Modal */}
      {pairingModalDev && (
        <div className="modal-overlay" onClick={() => setPairingModalDev(null)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "14px" }}>
              <Shield size={20} color="#06b6d4" />
              <h3 style={{ fontSize: "16px", fontWeight: 700 }}>Pairing with {pairingModalDev.name}</h3>
            </div>

            {/* PIN display */}
            <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "8px" }}>
              Security PIN — this code will appear on <strong>{pairingModalDev.name}</strong> ({pairingModalDev.ip_address}).
              Verify it matches before accepting there.
            </p>

            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: "32px",
                fontWeight: 700,
                letterSpacing: "8px",
                textAlign: "center",
                padding: "18px",
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
                marginBottom: "16px",
                transition: "border-color 0.3s, color 0.3s",
              }}
            >
              {pairCode}
            </div>

            {/* Status messages */}
            {!isPairingLoading && !pairingResult && (
              <p style={{ fontSize: "12px", color: "var(--text-muted)", textAlign: "center", marginBottom: "14px" }}>
                Click <strong>Send Request</strong> — the remote machine will show a confirmation dialog.
              </p>
            )}
            {isPairingLoading && (
              <p style={{ fontSize: "13px", color: "#06b6d4", textAlign: "center", marginBottom: "14px" }}>
                ⏳ Waiting for {pairingModalDev.name} to accept… (up to 30 seconds)
              </p>
            )}
            {pairingResult === "accepted" && (
              <p style={{ fontSize: "13px", color: "#10b981", textAlign: "center", marginBottom: "14px", fontWeight: 600 }}>
                ✅ Paired successfully! Device added to your ring.
              </p>
            )}
            {pairingResult === "rejected" && (
              <div style={{
                marginBottom: "14px",
                background: "rgba(244,63,94,0.06)",
                border: "1px solid rgba(244,63,94,0.25)",
                borderRadius: "var(--radius-sm)",
                padding: "12px 14px",
              }}>
                <p style={{ fontSize: "13px", color: "#f43f5e", fontWeight: 600, marginBottom: "6px" }}>
                  ❌ {pairingError ? "Cannot reach device" : "Pairing rejected or timed out"}
                </p>
                {pairingError ? (
                  <pre style={{
                    fontSize: "11px",
                    color: "var(--text-secondary)",
                    whiteSpace: "pre-wrap",
                    fontFamily: "var(--font-mono)",
                    margin: 0,
                  }}>
                    {pairingError}
                  </pre>
                ) : (
                  <p style={{ fontSize: "12px", color: "var(--text-muted)", margin: 0 }}>
                    The remote user declined or did not respond in time.
                  </p>
                )}
              </div>
            )}

            <div style={{ display: "flex", gap: "10px" }}>
              <button
                className="btn btn-outline"
                onClick={() => { setPairingModalDev(null); setPairingResult(null); setPairingError(null); }}
                style={{ flex: 1 }}
                disabled={isPairingLoading}
              >
                {pairingResult === "accepted" ? "Close" : "Cancel"}
              </button>
              {!pairingResult && (
                <button
                  className="btn btn-primary"
                  onClick={confirmPairing}
                  disabled={isPairingLoading}
                  style={{ flex: 1 }}
                >
                  {isPairingLoading ? "Checking…" : "Send Request"}
                </button>
              )}
              {pairingResult === "rejected" && (
                <button
                  className="btn btn-primary"
                  onClick={() => { setPairingResult(null); setPairingError(null); confirmPairing(); }}
                  style={{ flex: 1 }}
                >
                  Retry
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
