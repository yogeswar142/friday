import React, { useState } from "react";
import {
  ArrowLeftRight,
  ArrowRight,
  Check,
  ChevronLeft,
  ChevronRight,
  Laptop,
  Plus,
  RotateCw,
  Trash2,
  Zap,
} from "lucide-react";
import { DeviceInfo, LocalDeviceDto, TopologyDto } from "../types";

interface CircularTopologyEditorProps {
  topology: TopologyDto | null;
  devices: DeviceInfo[];
  localDevice?: LocalDeviceDto | null;
  activeDeviceId: string;
  onUpdateRing: (newRing: string[]) => void;
  onSelectActiveOwner: (deviceId: string) => void;
}

export const CircularTopologyEditor: React.FC<CircularTopologyEditorProps> = ({
  topology,
  devices,
  localDevice,
  activeDeviceId,
  onUpdateRing,
  onSelectActiveOwner,
}) => {
  const [selectedDirection, setSelectedDirection] = useState<"right" | "left">("right");
  const [addDeviceModalOpen, setAddDeviceModalOpen] = useState(false);

  const ring = topology?.ring || [];

  const getDeviceDisplayName = (id: string): string => {
    const dev = devices.find((d) => d.id === id || d.ip_address === id);
    if (dev?.name) {
      return dev.name.replace(/ \(This Machine\)$/, "");
    }
    if (localDevice && (id === localDevice.device_id || id === localDevice.hostname)) {
      return localDevice.display_name;
    }
    return id.length > 12 ? `${id.slice(0, 8)}...` : id;
  };

  // Move node clockwise / right in ring
  const handleMoveRight = (index: number) => {
    if (ring.length <= 1) return;
    const newRing = [...ring];
    const targetIdx = (index + 1) % ring.length;
    const temp = newRing[index];
    newRing[index] = newRing[targetIdx];
    newRing[targetIdx] = temp;
    onUpdateRing(newRing);
  };

  // Move node counter-clockwise / left in ring
  const handleMoveLeft = (index: number) => {
    if (ring.length <= 1) return;
    const newRing = [...ring];
    const targetIdx = (index - 1 + ring.length) % ring.length;
    const temp = newRing[index];
    newRing[index] = newRing[targetIdx];
    newRing[targetIdx] = temp;
    onUpdateRing(newRing);
  };

  // Remove node from circular ring
  const handleRemoveNode = (nodeId: string) => {
    if (ring.length <= 2) {
      alert("A circular ring topology requires at least 2 connected nodes.");
      return;
    }
    const newRing = ring.filter((id) => id !== nodeId);
    onUpdateRing(newRing);
  };

  // Add available device to circular ring
  const handleAddDevice = (deviceId: string) => {
    if (!ring.includes(deviceId)) {
      onUpdateRing([...ring, deviceId]);
    }
    setAddDeviceModalOpen(false);
  };

  // Geometry for interactive circular SVG canvas
  const centerX = 300;
  const centerY = 230;
  const radius = 150;

  // Calculate coordinates for nodes around circle
  const nodeCoords = ring.map((id, index) => {
    // Start from top (-PI/2) and space evenly around 2*PI
    const angle = (2 * Math.PI * index) / ring.length - Math.PI / 2;
    const x = centerX + radius * Math.cos(angle);
    const y = centerY + radius * Math.sin(angle);
    return { id, x, y, angle };
  });

  const availableToAdd = devices.filter((d) => !ring.includes(d.id));

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* Top Header & Direction Mode Control */}
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
            <RotateCw size={18} color="#06b6d4" />
            Interactive Circular Ring Topology
          </h2>
          <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "4px" }}>
            Connected machines are organized in an infinite loop. Moving past any edge transfers control to the next node.
          </p>
        </div>

        <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
          {/* Direction toggle */}
          <div
            style={{
              display: "flex",
              background: "var(--bg-secondary)",
              padding: "2px",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--border-subtle)",
            }}
          >
            <button
              onClick={() => setSelectedDirection("right")}
              style={{
                padding: "6px 12px",
                fontSize: "12px",
                fontWeight: 600,
                borderRadius: "var(--radius-sm)",
                background: selectedDirection === "right" ? "var(--bg-active)" : "transparent",
                color: selectedDirection === "right" ? "var(--text-primary)" : "var(--text-muted)",
              }}
            >
              Right Loop (A → B → C)
            </button>
            <button
              onClick={() => setSelectedDirection("left")}
              style={{
                padding: "6px 12px",
                fontSize: "12px",
                fontWeight: 600,
                borderRadius: "var(--radius-sm)",
                background: selectedDirection === "left" ? "var(--bg-active)" : "transparent",
                color: selectedDirection === "left" ? "var(--text-primary)" : "var(--text-muted)",
              }}
            >
              Left Loop (A → C → B)
            </button>
          </div>

          <button
            className="btn btn-sm btn-primary"
            onClick={() => setAddDeviceModalOpen(true)}
            disabled={availableToAdd.length === 0}
          >
            <Plus size={14} />
            <span>Add Node</span>
          </button>
        </div>
      </div>

      {/* Circular Radar Visualizer Canvas */}
      <div className="topology-container" style={{ position: "relative" }}>
        <svg className="topology-svg" viewBox="0 0 600 460">
          <defs>
            <marker
              id="arrowhead-right"
              markerWidth="10"
              markerHeight="7"
              refX="8"
              refY="3.5"
              orient="auto"
            >
              <polygon points="0 0, 10 3.5, 0 7" fill="#06b6d4" />
            </marker>
            <marker
              id="arrowhead-left"
              markerWidth="10"
              markerHeight="7"
              refX="2"
              refY="3.5"
              orient="auto"
            >
              <polygon points="10 0, 0 3.5, 10 7" fill="#f59e0b" />
            </marker>
          </defs>

          {/* Central Circular Orbital Ring */}
          <circle
            cx={centerX}
            cy={centerY}
            r={radius}
            fill="none"
            stroke="var(--border-subtle)"
            strokeWidth="2"
            strokeDasharray="6 6"
          />

          {/* Directed Circular Route Curves */}
          {nodeCoords.map((curr, idx) => {
            if (ring.length < 2) return null;
            const nextIdx =
              selectedDirection === "right"
                ? (idx + 1) % ring.length
                : (idx - 1 + ring.length) % ring.length;
            const target = nodeCoords[nextIdx];

            // Arc between nodes
            const midAngle = (curr.angle + target.angle) / 2;
            const labelX = centerX + (radius - 24) * Math.cos(midAngle);
            const labelY = centerY + (radius - 24) * Math.sin(midAngle);

            return (
              <g key={`link-${curr.id}-${target.id}`}>
                <line
                  x1={curr.x}
                  y1={curr.y}
                  x2={target.x}
                  y2={target.y}
                  stroke={selectedDirection === "right" ? "#06b6d4" : "#f59e0b"}
                  strokeWidth="2"
                  markerEnd={selectedDirection === "right" ? "url(#arrowhead-right)" : "url(#arrowhead-left)"}
                  strokeOpacity="0.75"
                />
              </g>
            );
          })}

          {/* Center Hub Indicator */}
          <circle cx={centerX} cy={centerY} r="38" fill="var(--bg-card)" stroke="var(--border-subtle)" strokeWidth="1" />
          <text
            x={centerX}
            y={centerY - 6}
            textAnchor="middle"
            fill="var(--accent-cyan)"
            fontSize="10"
            fontFamily="JetBrains Mono"
            fontWeight="700"
          >
            ROUTING
          </text>
          <text
            x={centerX}
            y={centerY + 10}
            textAnchor="middle"
            fill="var(--text-muted)"
            fontSize="9"
            fontFamily="Inter"
          >
            {selectedDirection === "right" ? "CLOCKWISE" : "COUNTER-CW"}
          </text>
        </svg>

        {/* Interactive Device Node Badges positioned on canvas */}
        {nodeCoords.map((node, index) => {
          const dev = devices.find((d) => d.id === node.id);
          const isActive = node.id === activeDeviceId;

          return (
            <div
              key={node.id}
              style={{
                position: "absolute",
                left: `${node.x}px`,
                top: `${node.y}px`,
                transform: "translate(-50%, -50%)",
                background: isActive ? "rgba(16, 185, 129, 0.15)" : "var(--bg-card)",
                border: `2px solid ${isActive ? "var(--accent-emerald)" : "var(--border-subtle)"}`,
                borderRadius: "var(--radius-md)",
                padding: "10px 14px",
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                gap: "4px",
                minWidth: "120px",
                boxShadow: isActive ? "0 0 16px rgba(16, 185, 129, 0.3)" : "none",
                zIndex: 10,
                transition: "all 0.2s ease",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <Laptop size={16} color={isActive ? "#10b981" : "#94a3b8"} />
                <span style={{ fontWeight: 700, fontSize: "13px" }}>{getDeviceDisplayName(node.id)}</span>
              </div>

              <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                {dev ? dev.os : "Connected"}
              </span>

              {isActive ? (
                <span
                  style={{
                    fontSize: "10px",
                    fontWeight: 700,
                    color: "var(--accent-emerald)",
                    background: "rgba(16, 185, 129, 0.2)",
                    padding: "2px 6px",
                    borderRadius: "var(--radius-sm)",
                  }}
                >
                  ● ACTIVE OWNER
                </span>
              ) : (
                <button
                  className="btn btn-sm btn-outline"
                  onClick={() => onSelectActiveOwner(node.id)}
                  style={{ fontSize: "10px", padding: "2px 6px", marginTop: "2px" }}
                >
                  Give Focus
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Ring Order Editor & Reordering Controls */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <ArrowLeftRight size={16} color="#3b82f6" />
            Circular Order Sequence ({ring.length} Nodes)
          </span>
          <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>
            Reorder positions to alter screen edge boundary handoffs
          </span>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {ring.map((nodeId, index) => {
            const dev = devices.find((d) => d.id === nodeId);
            const isActive = nodeId === activeDeviceId;

            return (
              <div
                key={nodeId}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "10px 16px",
                  background: "var(--bg-secondary)",
                  borderRadius: "var(--radius-sm)",
                  border: `1px solid ${isActive ? "var(--accent-emerald)" : "var(--border-subtle)"}`,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "14px" }}>
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: "12px",
                      fontWeight: 700,
                      color: "var(--accent-cyan)",
                      width: "24px",
                    }}
                  >
                    #{index + 1}
                  </span>
                  <div>
                    <span style={{ fontWeight: 600, fontSize: "14px" }}>{getDeviceDisplayName(nodeId)}</span>
                    <span style={{ fontSize: "12px", color: "var(--text-muted)", marginLeft: "8px" }}>
                      {dev?.os || "Connected"} • {dev?.is_local ? "Main Host" : (dev?.connection_state || "Connected")}
                    </span>
                  </div>
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  {isActive && <span className="device-badge badge-active">ACTIVE MOUSE</span>}

                  <button
                    className="btn btn-sm btn-outline"
                    onClick={() => handleMoveLeft(index)}
                    title="Move counter-clockwise in ring"
                  >
                    <ChevronLeft size={14} />
                  </button>

                  <button
                    className="btn btn-sm btn-outline"
                    onClick={() => handleMoveRight(index)}
                    title="Move clockwise in ring"
                  >
                    <ChevronRight size={14} />
                  </button>

                  <button
                    className="btn btn-sm btn-outline"
                    onClick={() => handleRemoveNode(nodeId)}
                    title="Remove from ring"
                    style={{ color: "var(--accent-rose)" }}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Add Device Modal */}
      {addDeviceModalOpen && (
        <div className="modal-overlay" onClick={() => setAddDeviceModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h3 style={{ fontSize: "16px", fontWeight: 700, marginBottom: "12px" }}>
              Add Device to Circular Ring
            </h3>
            <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "16px" }}>
              Select a paired device to attach to the circular mouse routing ring:
            </p>

            <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginBottom: "20px" }}>
              {availableToAdd.map((dev) => (
                <div
                  key={dev.id}
                  onClick={() => handleAddDevice(dev.id)}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    padding: "10px 14px",
                    background: "var(--bg-secondary)",
                    borderRadius: "var(--radius-sm)",
                    border: "1px solid var(--border-subtle)",
                    cursor: "pointer",
                  }}
                >
                  <div>
                    <div style={{ fontWeight: 600 }}>{dev.name}</div>
                    <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{dev.os} • {dev.arch}</div>
                  </div>
                  <span className="btn btn-sm btn-primary">Add</span>
                </div>
              ))}
            </div>

            <button className="btn btn-outline" onClick={() => setAddDeviceModalOpen(false)} style={{ width: "100%" }}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
