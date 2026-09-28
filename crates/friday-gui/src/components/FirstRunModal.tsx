import React from "react";
import { Check, Laptop, RotateCw, Sparkles } from "lucide-react";

interface FirstRunModalProps {
  isOpen: boolean;
  onClose: () => void;
  onStartDiscovery: () => void;
}

export const FirstRunModal: React.FC<FirstRunModalProps> = ({
  isOpen,
  onClose,
  onStartDiscovery,
}) => {
  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()} style={{ maxWidth: "480px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "12px", marginBottom: "16px" }}>
          <div
            style={{
              padding: "10px",
              background: "rgba(6, 182, 212, 0.12)",
              borderRadius: "var(--radius-md)",
              border: "1px solid rgba(6, 182, 212, 0.3)",
            }}
          >
            <RotateCw size={24} color="#06b6d4" />
          </div>
          <div>
            <h2 style={{ fontSize: "18px", fontWeight: 700 }}>Welcome to FRIDAY</h2>
            <p style={{ fontSize: "13px", color: "var(--accent-cyan)", fontWeight: 500 }}>
              One mouse. N devices. One continuous loop.
            </p>
          </div>
        </div>

        <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "18px", lineHeight: 1.6 }}>
          FRIDAY links multiple computers into a unified circular peripheral plane. Seamlessly glide your physical mouse across Windows, Linux, and macOS without extra hardware.
        </p>

        <div
          style={{
            background: "var(--bg-secondary)",
            borderRadius: "var(--radius-sm)",
            padding: "16px",
            border: "1px solid var(--border-subtle)",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
            marginBottom: "22px",
            fontSize: "13px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span style={{ fontFamily: "var(--font-mono)", fontWeight: 700, color: "var(--accent-cyan)" }}>1.</span>
            <span>Install FRIDAY on all your laptops and workstations</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span style={{ fontFamily: "var(--font-mono)", fontWeight: 700, color: "var(--accent-cyan)" }}>2.</span>
            <span>Discover nearby devices on your Wi-Fi or LAN</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span style={{ fontFamily: "var(--font-mono)", fontWeight: 700, color: "var(--accent-cyan)" }}>3.</span>
            <span>Pair devices with one-click secure PIN verification</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span style={{ fontFamily: "var(--font-mono)", fontWeight: 700, color: "var(--accent-cyan)" }}>4.</span>
            <span>Arrange your circular ring topology (A → B → C → A)</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <span style={{ fontFamily: "var(--font-mono)", fontWeight: 700, color: "var(--accent-emerald)" }}>5.</span>
            <span style={{ fontWeight: 600 }}>Start FRIDAY and move your cursor freely!</span>
          </div>
        </div>

        <div style={{ display: "flex", gap: "10px" }}>
          <button className="btn btn-outline" onClick={onClose} style={{ flex: 1 }}>
            Skip to Dashboard
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              onClose();
              onStartDiscovery();
            }}
            style={{ flex: 1.3 }}
          >
            <Sparkles size={14} />
            <span>Discover Devices</span>
          </button>
        </div>
      </div>
    </div>
  );
};
