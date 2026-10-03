import React, { useEffect, useState } from "react";
import {
  CheckCircle2,
  Copy,
  Cpu,
  Filter,
  Gauge,
  Keyboard,
  Layers,
  MousePointer,
  Play,
  RotateCw,
  Terminal,
  Trash2,
  Wifi,
  XCircle,
  Zap,
} from "lucide-react";
import { BenchmarkReport, DiagnosticReport, LogEntry, NetworkWorkingReportDto } from "../types";
import { backendApi } from "../api/backend";

interface DiagnosticsProps {
  report: DiagnosticReport | null;
  benchmark: BenchmarkReport | null;
  logs: LogEntry[];
  onClearLogs?: () => void;
  onRunDiagnostics: () => void;
  onRunBenchmark: () => void;
  isLoadingDiag: boolean;
  isLoadingBench: boolean;
}

export const Diagnostics: React.FC<DiagnosticsProps> = ({
  report,
  benchmark,
  logs,
  onClearLogs,
  onRunDiagnostics,
  onRunBenchmark,
  isLoadingDiag,
  isLoadingBench,
}) => {
  const [copied, setCopied] = useState(false);
  const [showLogTerminal, setShowLogTerminal] = useState(true);
  const [logFilter, setLogFilter] = useState<"all" | "mouse" | "keyboard" | "pairing">("all");
  const [copiedLogs, setCopiedLogs] = useState(false);

  const [netReport, setNetReport] = useState<NetworkWorkingReportDto | null>(null);
  const [copiedNetReport, setCopiedNetReport] = useState(false);
  const [showNetTimeline, setShowNetTimeline] = useState(false);

  const fetchNetReport = async () => {
    try {
      const res = await backendApi.getNetworkWorkingReport();
      setNetReport(res);
    } catch (e) {
      console.error("Failed to fetch dynamic network report:", e);
    }
  };

  useEffect(() => {
    fetchNetReport();
    const interval = setInterval(fetchNetReport, 1500);
    return () => clearInterval(interval);
  }, []);

  const copyFullNetworkReport = () => {
    if (!netReport) return;
    navigator.clipboard.writeText(netReport.formatted_report);
    setCopiedNetReport(true);
    setTimeout(() => setCopiedNetReport(false), 2000);
  };


  const copyDiagnosticReport = () => {
    if (!report) return;
    const text = [
      `=== FRIDAY DIAGNOSTIC REPORT ===`,
      `Engine State: ${report.engine_state}`,
      `Platform: ${report.platform} (${report.architecture})`,
      `Display / Desktop: ${report.os_version}`,
      `Input Backend: ${report.input_backend}`,
      `Network: ${report.network_transport}`,
      `Latency: ${report.latency_ms.toFixed(2)} ms`,
      `Packet Loss: ${report.packet_loss_pct}%`,
      `Active Device: ${report.active_device}`,
      `Circular Ring: ${report.ring_nodes.join(" -> ")}`,
      `Topology Valid: ${report.topology_valid}`,
      `Checks:`,
      ...report.checks.map((c) => `  [${c.passed ? "PASS" : "FAIL"}] ${c.name}: ${c.detail}`),
      benchmark
        ? `\nBenchmark: Serialization: ${benchmark.serialization_throughput_kops.toFixed(1)} kops/s | Routing Decision: ${benchmark.routing_decision_latency_ns.toFixed(1)} ns | Rating: ${benchmark.rating}`
        : "",
    ].join("\n");

    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* Action Toolbar */}
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
            <Layers size={18} color="#06b6d4" />
            System Diagnostics & Latency Benchmark
          </h2>
          <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
            Real-time inspection of low-latency data plane, display server bindings, and routing performance
          </p>
        </div>

        <div style={{ display: "flex", gap: "8px" }}>
          <button
            className="btn btn-outline"
            onClick={onRunDiagnostics}
            disabled={isLoadingDiag}
          >
            <RotateCw size={14} className={isLoadingDiag ? "spin" : ""} />
            <span>{isLoadingDiag ? "Diagnosing..." : "Run Diagnostics"}</span>
          </button>

          <button
            className="btn btn-outline"
            onClick={onRunBenchmark}
            disabled={isLoadingBench}
          >
            <Gauge size={14} />
            <span>{isLoadingBench ? "Benchmarking..." : "Run Benchmark"}</span>
          </button>

          <button
            className="btn btn-primary"
            onClick={copyDiagnosticReport}
            disabled={!report}
          >
            <Copy size={14} />
            <span>{copied ? "Copied Report!" : "Copy Report"}</span>
          </button>
        </div>
      </div>

      {/* ── Dynamic Network Working Report Card ────────────────────── */}
      <div
        style={{
          background: "linear-gradient(135deg, rgba(59, 130, 246, 0.08) 0%, rgba(18, 21, 28, 1) 100%)",
          border: "1px solid rgba(59, 130, 246, 0.3)",
          borderRadius: "var(--radius-md)",
          padding: "20px 24px",
          display: "flex",
          flexDirection: "column",
          gap: "16px",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "10px" }}>
          <div>
            <span style={{ fontSize: "15px", fontWeight: 700, color: "var(--accent-blue)", display: "flex", alignItems: "center", gap: "8px" }}>
              <Wifi size={18} />
              Dynamic Network Working Report & Telemetry Log
            </span>
            <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }}>
              Full session dynamic metrics covering packet delivery, WiFi jitter, stall detection, and operational timeline
            </p>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            {netReport && (
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "11px",
                  padding: "4px 10px",
                  borderRadius: "var(--radius-sm)",
                  fontWeight: 600,
                  background: netReport.stall_count === 0 ? "rgba(16, 185, 129, 0.2)" : "rgba(245, 158, 11, 0.2)",
                  color: netReport.stall_count === 0 ? "var(--accent-emerald)" : "var(--accent-amber)",
                  border: `1px solid ${netReport.stall_count === 0 ? "rgba(16, 185, 129, 0.4)" : "rgba(245, 158, 11, 0.4)"}`,
                }}
              >
                {netReport.network_health}
              </span>
            )}
            <button
              className="btn btn-primary btn-sm"
              onClick={copyFullNetworkReport}
              disabled={!netReport}
            >
              <Copy size={13} />
              <span>{copiedNetReport ? "Copied Full Report!" : "Copy Full Network Report"}</span>
            </button>
          </div>
        </div>

        {netReport && (
          <>
            <div className="grid-4" style={{ marginTop: "4px" }}>
              <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>SESSION DURATION</div>
                <div style={{ fontFamily: "var(--font-mono)", fontSize: "17px", fontWeight: 700, color: "var(--text-primary)", marginTop: "4px" }}>
                  {netReport.session_duration}
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "2px" }}>Started {netReport.session_start_time}</div>
              </div>

              <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>TOTAL PACKETS (TX / RX)</div>
                <div style={{ fontFamily: "var(--font-mono)", fontSize: "17px", fontWeight: 700, color: "var(--accent-cyan)", marginTop: "4px" }}>
                  {netReport.total_tx_packets} / {netReport.total_rx_packets}
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "2px" }}>
                  {((netReport.total_tx_bytes + netReport.total_rx_bytes) / 1048576).toFixed(2)} MB transferred
                </div>
              </div>

              <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>ROUND-TRIP LATENCY (RTT)</div>
                <div style={{ fontFamily: "var(--font-mono)", fontSize: "17px", fontWeight: 700, color: "var(--accent-emerald)", marginTop: "4px" }}>
                  {netReport.latency_ms.toFixed(2)} ms
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "2px" }}>
                  Min: {netReport.min_latency_ms.toFixed(2)} | Max: {netReport.max_latency_ms.toFixed(2)} ms
                </div>
              </div>

              <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>WIFI / SOCKET STALLS</div>
                <div style={{ fontFamily: "var(--font-mono)", fontSize: "17px", fontWeight: 700, color: netReport.stall_count === 0 ? "var(--accent-emerald)" : "var(--accent-rose)", marginTop: "4px" }}>
                  {netReport.stall_count} {netReport.stall_count === 1 ? "stall" : "stalls"}
                </div>
                <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "2px" }}>
                  {netReport.stall_count > 0 ? `Max stall: ${netReport.max_stall_ms}ms` : "Zero packet delivery gaps"}
                </div>
              </div>
            </div>

            {/* Input Traffic Breakdown Cards */}
            <div className="grid-2">
              <div style={{ background: "var(--bg-card)", padding: "14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "12px", fontWeight: 700, color: "var(--accent-cyan)", marginBottom: "8px", display: "flex", alignItems: "center", gap: "6px" }}>
                  <MousePointer size={14} />
                  Transmitted Traffic (Host: {netReport.local_device_name})
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: "6px", fontSize: "12px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Mouse Motion Packets</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.tx_breakdown.mouse_moves}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Mouse Clicks / Scrolls</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.tx_breakdown.mouse_buttons}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Keyboard Keystrokes</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600, color: "var(--accent-emerald)" }}>{netReport.tx_breakdown.keyboard_events}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Control & Heartbeat Pings</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.tx_breakdown.control_packets}</span>
                  </div>
                </div>
              </div>

              <div style={{ background: "var(--bg-card)", padding: "14px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <div style={{ fontSize: "12px", fontWeight: 700, color: "var(--accent-emerald)", marginBottom: "8px", display: "flex", alignItems: "center", gap: "6px" }}>
                  <Keyboard size={14} />
                  Received Traffic (Client: {netReport.local_device_name})
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: "6px", fontSize: "12px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Mouse Moves Injected</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.rx_breakdown.mouse_moves}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Clicks / Scrolls Injected</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.rx_breakdown.mouse_buttons}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Keystrokes Injected</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600, color: "var(--accent-emerald)" }}>{netReport.rx_breakdown.keyboard_events}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Control Handshakes Injected</span>
                    <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600 }}>{netReport.rx_breakdown.control_packets}</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Toggle Timeline */}
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <button
                className="btn btn-outline btn-sm"
                onClick={() => setShowNetTimeline(!showNetTimeline)}
              >
                <span>{showNetTimeline ? "Hide Event Timeline" : `View Operational Timeline (${netReport.timeline.length} events)`}</span>
              </button>
            </div>

            {showNetTimeline && (
              <div
                style={{
                  background: "var(--bg-secondary)",
                  border: "1px solid var(--border-subtle)",
                  borderRadius: "var(--radius-sm)",
                  padding: "10px 14px",
                  maxHeight: "220px",
                  overflowY: "auto",
                  fontFamily: "var(--font-mono)",
                  fontSize: "11px",
                  display: "flex",
                  flexDirection: "column",
                  gap: "6px",
                }}
              >
                {netReport.timeline.length === 0 ? (
                  <div style={{ color: "var(--text-muted)" }}>No operational events recorded yet</div>
                ) : (
                  netReport.timeline.map((item, idx) => (
                    <div key={idx} style={{ display: "flex", gap: "10px", alignItems: "baseline" }}>
                      <span style={{ color: "var(--text-muted)", flexShrink: 0 }}>[{item.timestamp}]</span>
                      <span
                        style={{
                          fontWeight: 700,
                          flexShrink: 0,
                          color:
                            item.level === "WARN" || item.level === "STALL"
                              ? "var(--accent-amber)"
                              : item.level === "ERROR"
                              ? "var(--accent-rose)"
                              : "var(--accent-cyan)",
                        }}
                      >
                        {item.level}
                      </span>
                      <span style={{ color: "var(--text-secondary)", flexShrink: 0 }}>[{item.category}]</span>
                      <span style={{ color: "var(--text-primary)" }}>{item.message}</span>
                    </div>
                  ))
                )}
              </div>
            )}
          </>
        )}
      </div>

      {/* Benchmark Summary Card if run */}
      {benchmark && (
        <div
          style={{
            background: "linear-gradient(135deg, rgba(6, 182, 212, 0.08) 0%, rgba(18, 21, 28, 1) 100%)",
            border: "1px solid rgba(6, 182, 212, 0.3)",
            borderRadius: "var(--radius-md)",
            padding: "18px 24px",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "12px" }}>
            <span style={{ fontSize: "14px", fontWeight: 700, color: "var(--accent-cyan)", display: "flex", alignItems: "center", gap: "8px" }}>
              <Zap size={16} />
              Core Routing Micro-Benchmark Results
            </span>
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: "11px",
                background: "rgba(16, 185, 129, 0.2)",
                color: "var(--accent-emerald)",
                padding: "3px 8px",
                borderRadius: "var(--radius-sm)",
                fontWeight: 600,
              }}
            >
              {benchmark.rating}
            </span>
          </div>

          <div className="grid-3" style={{ marginTop: "12px" }}>
            <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
              <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>SERIALIZATION SPEED</div>
              <div style={{ fontFamily: "var(--font-mono)", fontSize: "18px", fontWeight: 700, color: "var(--text-primary)", marginTop: "4px" }}>
                {benchmark.serialization_throughput_kops.toFixed(1)} kops/s
              </div>
              <div style={{ fontSize: "11px", color: "var(--accent-emerald)", marginTop: "2px" }}>Bincode zero-copy</div>
            </div>

            <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
              <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>COORDINATE TRANSFORMATION</div>
              <div style={{ fontFamily: "var(--font-mono)", fontSize: "18px", fontWeight: 700, color: "var(--text-primary)", marginTop: "4px" }}>
                {benchmark.coord_transform_latency_ns.toFixed(1)} ns
              </div>
              <div style={{ fontSize: "11px", color: "var(--accent-cyan)", marginTop: "2px" }}>Proportional normalization</div>
            </div>

            <div style={{ background: "var(--bg-card)", padding: "12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
              <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>ROUTING DECISION LATENCY</div>
              <div style={{ fontFamily: "var(--font-mono)", fontSize: "18px", fontWeight: 700, color: "var(--text-primary)", marginTop: "4px" }}>
                {benchmark.routing_decision_latency_ns.toFixed(1)} ns
              </div>
              <div style={{ fontSize: "11px", color: "var(--accent-emerald)", marginTop: "2px" }}>Sub-microsecond transfer</div>
            </div>
          </div>
        </div>
      )}

      {/* Main Diagnostic Data */}
      {report && (
        <div className="grid-2">
          {/* Left: Environment and Spec Parameters */}
          <div className="card" style={{ gap: "14px" }}>
            <h3 className="card-title">
              <Cpu size={16} color="#3b82f6" />
              Runtime Architecture & Environment
            </h3>

            <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "13px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--border-subtle)" }}>
                <span style={{ color: "var(--text-muted)" }}>Operating System</span>
                <span style={{ fontWeight: 600 }}>{report.platform} ({report.architecture})</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--border-subtle)" }}>
                <span style={{ color: "var(--text-muted)" }}>Display Server</span>
                <span style={{ fontWeight: 600 }}>{report.os_version}</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--border-subtle)" }}>
                <span style={{ color: "var(--text-muted)" }}>Input Capture / Injection</span>
                <span style={{ fontWeight: 600 }}>{report.input_backend}</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--border-subtle)" }}>
                <span style={{ color: "var(--text-muted)" }}>Network Transport Layer</span>
                <span style={{ fontWeight: 600, color: "var(--accent-cyan)" }}>{report.network_transport}</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--border-subtle)" }}>
                <span style={{ color: "var(--text-muted)" }}>Active Mouse Owner</span>
                <span style={{ fontWeight: 700, color: "var(--accent-emerald)" }}>{report.active_device}</span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0" }}>
                <span style={{ color: "var(--text-muted)" }}>Circular Topology Sequence</span>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: "12px", color: "var(--accent-cyan)" }}>
                  {report.ring_nodes.join(" → ")}
                </span>
              </div>
            </div>
          </div>

          {/* Right: Validation Checks List */}
          <div className="card" style={{ gap: "14px" }}>
            <div className="card-header">
              <h3 className="card-title">
                <CheckCircle2 size={16} color="#10b981" />
                Diagnostic Health Checks
              </h3>
              <button
                className="btn btn-sm btn-outline"
                onClick={() => setShowLogTerminal(!showLogTerminal)}
              >
                <Terminal size={12} />
                <span>{showLogTerminal ? "Hide Logs" : "View Logs"}</span>
              </button>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
              {report.checks.map((c, i) => (
                <div
                  key={i}
                  style={{
                    display: "flex",
                    alignItems: "flex-start",
                    gap: "10px",
                    padding: "10px 12px",
                    background: "var(--bg-secondary)",
                    borderRadius: "var(--radius-sm)",
                    border: `1px solid ${c.passed ? "var(--border-subtle)" : "rgba(244, 63, 94, 0.4)"}`,
                  }}
                >
                  {c.passed ? (
                    <CheckCircle2 size={16} color="#10b981" style={{ marginTop: "2px", flexShrink: 0 }} />
                  ) : (
                    <XCircle size={16} color="#f43f5e" style={{ marginTop: "2px", flexShrink: 0 }} />
                  )}
                  <div>
                    <div style={{ fontWeight: 600, fontSize: "13px" }}>{c.name}</div>
                    <div style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>{c.detail}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* ── Unified Diagnostic & Mouse Movement Logs ─────────────────── */}
      {(() => {
        const mouseLogs = logs.filter(
          (l) =>
            l.target.includes("mouse") ||
            l.target.includes("ownership") ||
            l.message.toLowerCase().includes("mouse") ||
            l.message.toLowerCase().includes("cursor") ||
            l.message.toLowerCase().includes("dwell") ||
            l.message.toLowerCase().includes("handoff") ||
            l.message.toLowerCase().includes("edge")
        );

        const pairingLogs = logs.filter(
          (l) =>
            l.target.includes("pairing") ||
            l.message.toLowerCase().includes("pairing") ||
            l.message.toLowerCase().includes("probe") ||
            l.message.toLowerCase().includes("pair") ||
            l.message.toLowerCase().includes("connected to") ||
            l.message.toLowerCase().includes("unpaired")
        );

        const keyboardLogs = logs.filter(
          (l) =>
            l.target.includes("keyboard") ||
            l.message.toLowerCase().includes("keyboard") ||
            l.message.toLowerCase().includes("key ") ||
            l.message.toLowerCase().includes("injected key")
        );

        const activeLogs =
          logFilter === "mouse"
            ? mouseLogs
            : logFilter === "keyboard"
            ? keyboardLogs
            : logFilter === "pairing"
            ? pairingLogs
            : logs;

        const copyCurrentLogs = () => {
          const title =
            logFilter === "mouse"
              ? "FRIDAY MOUSE MOVEMENT LOGS"
              : logFilter === "keyboard"
              ? "FRIDAY KEYBOARD EVENT LOGS"
              : logFilter === "pairing"
              ? "FRIDAY PAIRING LOGS"
              : "FRIDAY ENGINE DIAGNOSTIC LOGS";
          const text =
            activeLogs.length === 0
              ? "(no logs in this filter)"
              : activeLogs
                  .map((l) => `[${l.timestamp}] ${l.level}  ${l.target}: ${l.message}`)
                  .join("\n");
          navigator.clipboard.writeText(`=== ${title} ===\n${text}\n===========================`);
          setCopiedLogs(true);
          setTimeout(() => setCopiedLogs(false), 2000);
        };

        return (
          <div className="card" style={{ gap: "12px" }}>
            <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "10px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <Terminal size={17} color="#06b6d4" />
                <span className="card-title" style={{ fontSize: "15px", fontWeight: 700 }}>
                  Diagnostic & Input Logs ({activeLogs.length})
                </span>
              </div>

              {/* Filter tabs */}
              <div style={{ display: "flex", alignItems: "center", gap: "6px", background: "rgba(0,0,0,0.3)", padding: "3px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)" }}>
                <button
                  className={`btn btn-sm ${logFilter === "all" ? "btn-primary" : "btn-ghost"}`}
                  onClick={() => setLogFilter("all")}
                  style={{ fontSize: "11px", padding: "4px 8px" }}
                >
                  All Logs ({logs.length})
                </button>
                <button
                  className={`btn btn-sm ${logFilter === "mouse" ? "btn-primary" : "btn-ghost"}`}
                  onClick={() => setLogFilter("mouse")}
                  style={{ fontSize: "11px", padding: "4px 8px", display: "flex", alignItems: "center", gap: "4px" }}
                >
                  <MousePointer size={11} />
                  Mouse ({mouseLogs.length})
                </button>
                <button
                  className={`btn btn-sm ${logFilter === "keyboard" ? "btn-primary" : "btn-ghost"}`}
                  onClick={() => setLogFilter("keyboard")}
                  style={{ fontSize: "11px", padding: "4px 8px", display: "flex", alignItems: "center", gap: "4px" }}
                >
                  <Keyboard size={11} />
                  Keyboard ({keyboardLogs.length})
                </button>
                <button
                  className={`btn btn-sm ${logFilter === "pairing" ? "btn-primary" : "btn-ghost"}`}
                  onClick={() => setLogFilter("pairing")}
                  style={{ fontSize: "11px", padding: "4px 8px", display: "flex", alignItems: "center", gap: "4px" }}
                >
                  <Wifi size={11} />
                  Pairing ({pairingLogs.length})
                </button>
              </div>

              {/* Actions: Copy & Clear */}
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <button
                  className="btn btn-sm btn-outline"
                  onClick={copyCurrentLogs}
                  title="Copy displayed logs to clipboard"
                  style={{ fontSize: "11px" }}
                >
                  <Copy size={12} />
                  <span>{copiedLogs ? "Copied!" : "Copy"}</span>
                </button>
                {onClearLogs && (
                  <button
                    className="btn btn-sm btn-outline"
                    onClick={onClearLogs}
                    title="Clear all stored logs"
                    style={{ fontSize: "11px", color: "#f43f5e", borderColor: "rgba(244,63,94,0.3)" }}
                  >
                    <Trash2 size={12} />
                    <span>Clear Logs</span>
                  </button>
                )}
              </div>
            </div>

            {activeLogs.length === 0 ? (
              <div style={{ textAlign: "center", padding: "24px 10px", color: "var(--text-muted)", fontSize: "13px" }}>
                {logFilter === "mouse"
                  ? "No mouse movement events recorded yet. Move your mouse or touchpad to screen edges to see live routing logs."
                  : logFilter === "keyboard"
                  ? "No keyboard events recorded yet. Type on your physical keyboard while controlling a remote machine to see live keystroke routing and injection logs."
                  : logFilter === "pairing"
                  ? "No pairing events yet. Connect or pair a machine to see handshake logs."
                  : "No diagnostic logs recorded yet."}
              </div>
            ) : (
              <div
                style={{
                  background: "#05070a",
                  padding: "14px",
                  borderRadius: "var(--radius-sm)",
                  fontFamily: "var(--font-mono)",
                  fontSize: "12px",
                  maxHeight: "360px",
                  overflowY: "auto",
                  display: "flex",
                  flexDirection: "column",
                  gap: "5px",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                {activeLogs.map((log, i) => {
                  const isMouse = log.target.includes("mouse") || log.target.includes("ownership");
                  const isKeyboard = log.target.includes("keyboard") || log.message.toLowerCase().includes("key");
                  const isPairing = log.target.includes("pairing") || log.message.toLowerCase().includes("pair");

                  return (
                    <div key={i} style={{ display: "flex", gap: "8px", alignItems: "flex-start", lineHeight: 1.4 }}>
                      <span style={{ color: "#64748b", flexShrink: 0 }}>[{log.timestamp}]</span>
                      <span
                        style={{
                          color:
                            log.level === "INFO" ? "#06b6d4"
                            : log.level === "WARN" ? "#f59e0b"
                            : "#f43f5e",
                          fontWeight: 600,
                          flexShrink: 0,
                          minWidth: "40px",
                        }}
                      >
                        {log.level}
                      </span>
                      <span
                        style={{
                          color: isMouse ? "#38bdf8" : isKeyboard ? "#f59e0b" : isPairing ? "#10b981" : "#a855f7",
                          flexShrink: 0,
                          fontWeight: 500,
                          fontSize: "11px",
                          background: isMouse
                            ? "rgba(56,189,248,0.1)"
                            : isKeyboard
                            ? "rgba(245,158,11,0.1)"
                            : isPairing
                            ? "rgba(16,185,129,0.1)"
                            : "rgba(168,85,247,0.1)",
                          padding: "1px 5px",
                          borderRadius: "3px",
                        }}
                      >
                        {log.target.replace("friday_core::", "").replace("friday_network::", "")}
                      </span>
                      <span style={{ color: "#f1f5f9", wordBreak: "break-word" }}>{log.message}</span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })()}
    </div>
  );
};
