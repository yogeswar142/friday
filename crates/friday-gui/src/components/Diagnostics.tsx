import React, { useState } from "react";
import {
  CheckCircle2,
  Copy,
  Cpu,
  Gauge,
  Layers,
  Play,
  RotateCw,
  Terminal,
  XCircle,
  Zap,
} from "lucide-react";
import { BenchmarkReport, DiagnosticReport, LogEntry } from "../types";

interface DiagnosticsProps {
  report: DiagnosticReport | null;
  benchmark: BenchmarkReport | null;
  logs: LogEntry[];
  onRunDiagnostics: () => void;
  onRunBenchmark: () => void;
  isLoadingDiag: boolean;
  isLoadingBench: boolean;
}

export const Diagnostics: React.FC<DiagnosticsProps> = ({
  report,
  benchmark,
  logs,
  onRunDiagnostics,
  onRunBenchmark,
  isLoadingDiag,
  isLoadingBench,
}) => {
  const [copied, setCopied] = useState(false);
  const [showLogTerminal, setShowLogTerminal] = useState(false);
  const [copiedPairing, setCopiedPairing] = useState(false);
  const [copiedAll, setCopiedAll] = useState(false);


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

      {/* ── Dedicated Pairing Logs ──────────────────────────────── */}
      {(() => {
        const pairingLogs = logs.filter((l) => l.target === "friday_network::pairing");

        const copyPairingLogs = () => {
          const text = pairingLogs.length === 0
            ? "(no pairing logs yet)"
            : pairingLogs
                .map((l) => `[${l.timestamp}] ${l.level}  ${l.message}`)
                .join("\n");
          navigator.clipboard.writeText(
            `=== FRIDAY PAIRING LOGS ===\n${text}\n===========================`
          );
          setCopiedPairing(true);
          setTimeout(() => setCopiedPairing(false), 2000);
        };

        return (
          <div className="card" style={{ gap: "10px" }}>
            <div className="card-header">
              <span className="card-title">
                <Terminal size={16} color="#06b6d4" />
                Pairing Logs ({pairingLogs.length} entries)
              </span>
              <button
                className="btn btn-sm btn-outline"
                onClick={copyPairingLogs}
                title="Copy pairing logs to clipboard"
              >
                <Copy size={12} />
                <span>{copiedPairing ? "Copied!" : "Copy Logs"}</span>
              </button>
            </div>

            {pairingLogs.length === 0 ? (
              <p style={{ fontSize: "12px", color: "var(--text-muted)", padding: "8px 0" }}>
                No pairing events yet. Trigger a pairing attempt to see detailed step-by-step logs here.
              </p>
            ) : (
              <div
                style={{
                  background: "#05070a",
                  padding: "14px",
                  borderRadius: "var(--radius-sm)",
                  fontFamily: "var(--font-mono)",
                  fontSize: "12px",
                  maxHeight: "280px",
                  overflowY: "auto",
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                {pairingLogs.map((log, i) => (
                  <div key={i} style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
                    <span style={{ color: "#64748b", flexShrink: 0 }}>[{log.timestamp}]</span>
                    <span
                      style={{
                        color:
                          log.level === "INFO" ? "#06b6d4"
                          : log.level === "WARN" ? "#f59e0b"
                          : "#f43f5e",
                        fontWeight: 600,
                        flexShrink: 0,
                      }}
                    >
                      {log.level}
                    </span>
                    <span style={{ color: "#f1f5f9", whiteSpace: "pre-wrap" }}>{log.message}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      })()}

      {/* Internal Log Viewer (full engine log) */}
      {showLogTerminal && (
        <div className="card" style={{ gap: "10px" }}>
          <div className="card-header">
            <span className="card-title">
              <Terminal size={16} color="#06b6d4" />
              Engine Log Buffer ({logs.length} entries)
            </span>
            <button
              className="btn btn-sm btn-outline"
              onClick={() => {
                const text = logs
                  .map((l) => `[${l.timestamp}] ${l.level}  ${l.target}: ${l.message}`)
                  .join("\n");
                navigator.clipboard.writeText(`=== FRIDAY ENGINE LOGS ===\n${text}\n==========================`);
                setCopiedAll(true);
                setTimeout(() => setCopiedAll(false), 2000);
              }}
              title="Copy all engine logs"
            >
              <Copy size={12} />
              <span>{copiedAll ? "Copied!" : "Copy All"}</span>
            </button>
          </div>

          <div
            style={{
              background: "#05070a",
              padding: "14px",
              borderRadius: "var(--radius-sm)",
              fontFamily: "var(--font-mono)",
              fontSize: "12px",
              maxHeight: "240px",
              overflowY: "auto",
              display: "flex",
              flexDirection: "column",
              gap: "4px",
              border: "1px solid var(--border-subtle)",
            }}
          >
            {logs.map((log, i) => (
              <div key={i} style={{ display: "flex", gap: "8px" }}>
                <span style={{ color: "#64748b" }}>[{log.timestamp}]</span>
                <span
                  style={{
                    color: log.level === "INFO" ? "#06b6d4" : log.level === "WARN" ? "#f59e0b" : "#f43f5e",
                    fontWeight: 600,
                  }}
                >
                  {log.level}
                </span>
                <span style={{ color: "#94a3b8" }}>{log.target}:</span>
                <span style={{ color: "#f1f5f9" }}>{log.message}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
