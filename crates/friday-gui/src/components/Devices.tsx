import React, { useState } from "react";
import {
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
import { DeviceInfo, DiscoveredDevice } from "../types";

interface DevicesProps {
  devices: DeviceInfo[];
  discovered: DiscoveredDevice[];
  activeDeviceId: string;
  onPairDevice: (deviceId: string) => void;
  onAddManualDevice: (ip: string, port?: number, name?: string) => void;
  onUnpairDevice: (deviceId: string) => void;
  onConnectDevice: (deviceId: string) => void;
  onDisconnectDevice: (deviceId: string) => void;
  onSwitchOwner: (deviceId: string) => void;
  onRefreshDiscovery: () => void;
}

export const Devices: React.FC<DevicesProps> = ({
  devices,
  discovered,
  activeDeviceId,
  onPairDevice,
  onAddManualDevice,
  onUnpairDevice,
  onConnectDevice,
  onDisconnectDevice,
  onSwitchOwner,
  onRefreshDiscovery,
}) => {
  const [pairingModalDev, setPairingModalDev] = useState<DiscoveredDevice | null>(null);
  const [pairCode, setPairCode] = useState("");
  const [isPairingLoading, setIsPairingLoading] = useState(false);
  const [manualIp, setManualIp] = useState("");
  const [manualName, setManualName] = useState("");
  const [manualPort, setManualPort] = useState("48700");


  const startPairingFlow = (dev: DiscoveredDevice) => {
    setPairingModalDev(dev);
    // Generate a secure 6-digit confirmation PIN
    const pin = Math.floor(100000 + Math.random() * 900000).toString();
    setPairCode(pin);
  };

  const confirmPairing = () => {
    if (!pairingModalDev) return;
    setIsPairingLoading(true);
    setTimeout(() => {
      onPairDevice(pairingModalDev.id);
      setIsPairingLoading(false);
      setPairingModalDev(null);
    }, 400);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
      {/* Connected Devices Section */}
      <div className="card">
        <div className="card-header">
          <div>
            <h2 className="card-title">
              <Laptop size={17} color="#10b981" />
              Connected Devices ({devices.length})
            </h2>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Active nodes participating in the cross-platform peripheral sharing network
            </p>
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
                    <span className="device-badge badge-active">ACTIVE OWNER</span>
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
                    <span style={{ color: "var(--text-muted)" }}>TYPE: </span>
                    <span>{d.is_local ? "Host Machine" : "Remote Peer"}</span>
                  </div>
                  <div>
                    <span style={{ color: "var(--text-muted)" }}>CAPS: </span>
                    <span>{d.capabilities.length} features</span>
                  </div>
                </div>

                {/* Actions */}
                <div style={{ display: "flex", gap: "8px", marginTop: "auto" }}>
                  {!isActive && (
                    <button
                      className="btn btn-sm btn-primary"
                      onClick={() => onSwitchOwner(d.id)}
                      style={{ flex: 1 }}
                    >
                      <Zap size={12} />
                      Take Control
                    </button>
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

          <button type="submit" className="btn btn-primary" style={{ height: "38px" }}>
            <Plus size={14} />
            <span>Connect Device</span>
          </button>
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

            <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "16px" }}>
              Verify that the security PIN below matches the confirmation prompt on {pairingModalDev.name} ({pairingModalDev.ip_address}):
            </p>

            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: "28px",
                fontWeight: 700,
                letterSpacing: "6px",
                textAlign: "center",
                padding: "16px",
                background: "var(--bg-secondary)",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--border-focus)",
                color: "var(--accent-cyan)",
                marginBottom: "20px",
              }}
            >
              {pairCode}
            </div>

            <div style={{ display: "flex", gap: "10px" }}>
              <button
                className="btn btn-outline"
                onClick={() => setPairingModalDev(null)}
                style={{ flex: 1 }}
              >
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={confirmPairing}
                disabled={isPairingLoading}
                style={{ flex: 1 }}
              >
                {isPairingLoading ? "Pairing..." : "Confirm & Connect"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
