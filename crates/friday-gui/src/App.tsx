import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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
  LocalDeviceDto,
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
  const [localDevice, setLocalDevice] = useState<LocalDeviceDto | null>(null);
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

  // ── Event-driven pair request listener ──────────────────────────────────────
  // The Rust backend emits 'friday:pair_request' immediately when a remote device
  // sends a pairing request — no polling delay. This listener shows the modal instantly.
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<PendingPairRequest>("friday:pair_request", (event) => {
      const req = event.payload;
      setPendingRequests((prev) => {
        // Deduplicate: only add if no request with same PIN + device ID exists
        const alreadyExists = prev.some(
          (r) => r.pin === req.pin && r.from_id === req.from_id
        );
        if (alreadyExists) return prev;
        return [...prev, req];
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Initial load + polling fallback for state sync
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
      // Poll devices and topology to keep remote changes (unpair/pair/connect) in sync
      api.getDevices().then(setDevices).catch(() => {});
      api.getTopology().then(setTopology).catch(() => {});
      // Poll for incoming pair requests as fallback (primary path is Tauri event above)
      api.getPendingPairRequests().then(setPendingRequests).catch(() => {});
      // Poll logs so diagnostics log view updates in real time
      api.getLogs().then(setLogs).catch(() => {});
      // Poll local device identity
      api.getLocalDevice().then(setLocalDevice).catch(() => {});
    }, 2000);

    return () => clearInterval(interval);
  }, []);


  // Theme application
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const refreshAllData = async () => {
    try {
      const [s, t, devs, disc, topo, perms, caps, sett, l, localDev] = await Promise.all([
        api.getStatus(),
        api.getTelemetry(),
        api.getDevices(),
        api.discoverDevices(),
        api.getTopology(),
        api.getPlatformPermissions(),
        api.getPlatformCapabilities(),
        api.getSettings(),
        api.getLogs(),
        api.getLocalDevice(),
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
      setLocalDevice(localDev);

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
    try {
      await api.switchActiveDevice(deviceId);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to switch active owner:", e);
      alert(`Could not switch active owner: ${e}`);
    }
  };

  const handleUpdateRing = async (newRing: string[]) => {
    try {
      await api.setTopology(newRing);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to update topology:", e);
    }
  };

  const handleManualAddDevice = async (ip: string, port?: number, name?: string) => {
    try {
      await api.addManualDevice(ip, port, name);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to add device:", e);
      alert(`Failed to add device: ${e}`);
    }
  };

  const handleInitiatePairing = async (deviceId: string, pin: string, targetIp?: string): Promise<boolean> => {
    try {
      const accepted = await api.initiatePairing(deviceId, pin, targetIp);
      await refreshAllData();
      return accepted;
    } catch (e) {
      console.error("Pairing failed:", e);
      await refreshAllData();
      throw e;
    }
  };

  const handleRespondToPairRequest = async (pin: string, accept: boolean) => {
    try {
      await api.respondToPairRequest(pin, accept);
      // Remove from local list immediately
      setPendingRequests((prev) => prev.filter((r) => r.pin !== pin));
      await refreshAllData();
    } catch (e) {
      console.error("Failed to respond to pair request:", e);
    }
  };

  const handleUpdateLocalName = async (name: string) => {
    try {
      const updated = await api.setLocalDeviceName(name);
      setLocalDevice(updated);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to update local device name:", e);
    }
  };

  const handleUnpairDevice = async (deviceId: string) => {
    try {
      await api.unpairDevice(deviceId);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to unpair device:", e);
      alert(`Could not unpair device: ${e}`);
    }
  };

  const handleConnectDevice = async (deviceId: string) => {
    try {
      await api.connectDevice(deviceId);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to connect device:", e);
      alert(`Could not connect device: ${e}`);
    }
  };

  const handleDisconnectDevice = async (deviceId: string) => {
    try {
      await api.disconnectDevice(deviceId);
      await refreshAllData();
    } catch (e) {
      console.error("Failed to disconnect device:", e);
      alert(`Could not disconnect device: ${e}`);
    }
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

  const handleClearLogs = async () => {
    try {
      await api.clearLogs();
      const updated = await api.getLogs();
      setLogs(updated);
    } catch (e) {
      console.error("Failed to clear logs:", e);
    }
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
        devices={devices}
        localDevice={localDevice}
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
            localDevice={localDevice}
            onNavigateTab={setCurrentTab}
            onSwitchOwner={handleSwitchOwner}
            onRefresh={refreshAllData}
          />
        )}

        {currentTab === "devices" && (
          <Devices
            localDevice={localDevice}
            devices={devices}
            discovered={discovered}
            pendingRequests={pendingRequests}
            activeDeviceId={status?.active_device_id || ""}
            isHost={status?.is_host ?? true}
            engineRunning={status?.state === "running"}
            onInitiatePairing={handleInitiatePairing}
            onUnpairDevice={handleUnpairDevice}
            onConnectDevice={handleConnectDevice}
            onDisconnectDevice={handleDisconnectDevice}
            onSwitchOwner={handleSwitchOwner}
            onRefreshDiscovery={async () => {
              const d = await api.discoverDevices();
              setDiscovered(d);
            }}
            onRespondToPairRequest={handleRespondToPairRequest}
            onUpdateLocalName={handleUpdateLocalName}
            onNavigateToAdvancedNetwork={() => setCurrentTab("settings")}
          />
        )}

        {currentTab === "topology" && (
          <CircularTopologyEditor
            topology={topology}
            devices={devices}
            localDevice={localDevice}
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
            onClearLogs={handleClearLogs}
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
            onAddManualDevice={handleManualAddDevice}
            onInitiatePairing={handleInitiatePairing}
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
