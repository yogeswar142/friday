import React, { useEffect, useState } from "react";
import { Navbar } from "./components/Navbar";
import { Overview } from "./components/Overview";
import { CircularTopologyEditor } from "./components/CircularTopologyEditor";
import { Devices } from "./components/Devices";
import { Diagnostics } from "./components/Diagnostics";
import { Settings } from "./components/Settings";
import { FirstRunModal } from "./components/FirstRunModal";
import { api } from "./api/backend";
import {
  BenchmarkReport,
  DeviceInfo,
  DiagnosticReport,
  DiscoveredDevice,
  EngineStatus,
  LogEntry,
  PendingPairRequest,
  PlatformCapabilities,
  PlatformPermissions,
  SettingsDto,
  TabType,
  TelemetryDto,
  TopologyDto,
} from "./types";

export const App: React.FC = () => {
  const [currentTab, setCurrentTab] = useState<TabType>("overview");
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [telemetry, setTelemetry] = useState<TelemetryDto | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [discovered, setDiscovered] = useState<DiscoveredDevice[]>([]);
  const [pendingRequests, setPendingRequests] = useState<PendingPairRequest[]>([]);
  const [topology, setTopology] = useState<TopologyDto | null>(null);
  const [permissions, setPermissions] = useState<PlatformPermissions | null>(null);
  const [capabilities, setCapabilities] = useState<PlatformCapabilities | null>(null);
  const [settings, setSettings] = useState<SettingsDto | null>(null);
  const [diagnosticReport, setDiagnosticReport] = useState<DiagnosticReport | null>(null);
  const [benchmarkReport, setBenchmarkReport] = useState<BenchmarkReport | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isLoadingDiag, setIsLoadingDiag] = useState(false);
  const [isLoadingBench, setIsLoadingBench] = useState(false);
  const [firstRunModalOpen, setFirstRunModalOpen] = useState(false);
  const [theme, setTheme] = useState<"dark" | "light" | "system">("dark");

  // Initial load
  useEffect(() => {
    // Check first-run experience flag
    const hasSeenFirstRun = localStorage.getItem("friday_first_run_completed");
    if (!hasSeenFirstRun) {
      setFirstRunModalOpen(true);
    }

    // Load initial states
    refreshAllData();

    // Low-frequency polling for telemetry & active owner (never in the mouse path!)
    const interval = setInterval(() => {
      api.getStatus().then(setStatus).catch(() => {});
      api.getTelemetry().then(setTelemetry).catch(() => {});
      // Poll for incoming pair requests on this machine (from remote FRIDAY nodes)
      api.getPendingPairRequests().then(setPendingRequests).catch(() => {});
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  // Theme application
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const refreshAllData = async () => {
    try {
      const [s, t, devs, disc, topo, perms, caps, sett, l] = await Promise.all([
        api.getStatus(),
        api.getTelemetry(),
        api.getDevices(),
        api.discoverDevices(),
        api.getTopology(),
        api.getPlatformPermissions(),
        api.getPlatformCapabilities(),
        api.getSettings(),
        api.getLogs(),
      ]);

      setStatus(s);
      setTelemetry(t);
      setDevices(devs);
      setDiscovered(disc);
      setTopology(topo);
      setPermissions(perms);
      setCapabilities(caps);
      setSettings(sett);
      setLogs(l);

      // Auto-run initial diagnostics report
      api.runDiagnostics().then(setDiagnosticReport).catch(() => {});
    } catch (e) {
      console.error("Failed to load initial FRIDAY state:", e);
    }
  };

  const handleToggleEngine = async () => {
    if (!status) return;
    if (status.state === "running") {
      const next = await api.stopEngine();
      setStatus(next);
    } else {
      const next = await api.startEngine();
      setStatus(next);
    }
  };

  const handlePauseEngine = async () => {
    const next = await api.pauseEngine();
    setStatus(next);
  };

  const handleSwitchOwner = async (deviceId: string) => {
    await api.switchActiveDevice(deviceId);
    const nextStatus = await api.getStatus();
    setStatus(nextStatus);
    const nextDevs = await api.getDevices();
    setDevices(nextDevs);
  };

  const handleUpdateRing = async (newRing: string[]) => {
    await api.setTopology(newRing);
    const nextTopo = await api.getTopology();
    setTopology(nextTopo);
    const nextStatus = await api.getStatus();
    setStatus(nextStatus);
  };

  const handleManualAddDevice = async (ip: string, port?: number, name?: string) => {
    try {
      await api.addManualDevice(ip, port, name);
      refreshAllData();
    } catch (e) {
      console.error("Failed to add device:", e);
      alert(`Failed to add device: ${e}`);
    }
  };

  const handleInitiatePairing = async (deviceId: string, pin: string): Promise<boolean> => {
    try {
      const accepted = await api.initiatePairing(deviceId, pin);
      if (accepted) refreshAllData();
      return accepted;
    } catch (e) {
      console.error("Pairing failed:", e);
      return false;
    }
  };

  const handleRespondToPairRequest = async (pin: string, accept: boolean) => {
    try {
      await api.respondToPairRequest(pin, accept);
      // Remove from local list immediately
      setPendingRequests((prev) => prev.filter((r) => r.pin !== pin));
      if (accept) refreshAllData();
    } catch (e) {
      console.error("Failed to respond to pair request:", e);
    }
  };

  const handleUnpairDevice = async (deviceId: string) => {
    await api.unpairDevice(deviceId);
    refreshAllData();
  };

  const handleConnectDevice = async (deviceId: string) => {
    await api.connectDevice(deviceId);
    refreshAllData();
  };

  const handleDisconnectDevice = async (deviceId: string) => {
    await api.disconnectDevice(deviceId);
    refreshAllData();
  };

  const handleRunDiagnostics = async () => {
    setIsLoadingDiag(true);
    try {
      const rep = await api.runDiagnostics();
      setDiagnosticReport(rep);
    } finally {
      setIsLoadingDiag(false);
    }
  };

  const handleRunBenchmark = async () => {
    setIsLoadingBench(true);
    try {
      const bench = await api.runBenchmark();
      setBenchmarkReport(bench);
    } finally {
      setIsLoadingBench(false);
    }
  };

  const handleSaveSettings = async (newSettings: SettingsDto) => {
    await api.saveSettings(newSettings);
    setSettings(newSettings);
    setTheme(newSettings.appearance);
  };

  const handleCloseFirstRun = () => {
    localStorage.setItem("friday_first_run_completed", "true");
    setFirstRunModalOpen(false);
  };

  return (
    <div className="app-container">
      <Navbar
        currentTab={currentTab}
        onTabChange={setCurrentTab}
        status={status}
        onToggleEngine={handleToggleEngine}
        onPauseEngine={handlePauseEngine}
        theme={theme}
        onToggleTheme={() => setTheme(theme === "dark" ? "light" : "dark")}
      />

      <main className="main-content">
        {currentTab === "overview" && (
          <Overview
            status={status}
            telemetry={telemetry}
            devices={devices}
            permissions={permissions}
            ring={topology?.ring || []}
            onNavigateTab={setCurrentTab}
            onSwitchOwner={handleSwitchOwner}
            onRefresh={refreshAllData}
          />
        )}

        {currentTab === "devices" && (
          <Devices
            devices={devices}
            discovered={discovered}
            pendingRequests={pendingRequests}
            activeDeviceId={status?.active_device_id || ""}
            onInitiatePairing={handleInitiatePairing}
            onAddManualDevice={handleManualAddDevice}
            onUnpairDevice={handleUnpairDevice}
            onConnectDevice={handleConnectDevice}
            onDisconnectDevice={handleDisconnectDevice}
            onSwitchOwner={handleSwitchOwner}
            onRefreshDiscovery={async () => {
              const d = await api.discoverDevices();
              setDiscovered(d);
            }}
            onRespondToPairRequest={handleRespondToPairRequest}
          />
        )}

        {currentTab === "topology" && (
          <CircularTopologyEditor
            topology={topology}
            devices={devices}
            activeDeviceId={status?.active_device_id || ""}
            onUpdateRing={handleUpdateRing}
            onSelectActiveOwner={handleSwitchOwner}
          />
        )}

        {currentTab === "diagnostics" && (
          <Diagnostics
            report={diagnosticReport}
            benchmark={benchmarkReport}
            logs={logs}
            onRunDiagnostics={handleRunDiagnostics}
            onRunBenchmark={handleRunBenchmark}
            isLoadingDiag={isLoadingDiag}
            isLoadingBench={isLoadingBench}
          />
        )}

        {currentTab === "settings" && settings && (
          <Settings
            settings={settings}
            onSaveSettings={handleSaveSettings}
          />
        )}
      </main>

      <FirstRunModal
        isOpen={firstRunModalOpen}
        onClose={handleCloseFirstRun}
        onStartDiscovery={() => setCurrentTab("devices")}
      />
    </div>
  );
};
