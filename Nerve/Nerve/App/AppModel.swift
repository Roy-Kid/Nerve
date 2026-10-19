import AppKit
import Foundation
import SwiftUI

@MainActor
@Observable
final class AppModel {
    let store: JobStore
    let settings: SettingsStore
    let tunnels: MachineTunnelManager
    private let hubClient: HubClient
    private let hubProcessManager: HubProcessManager
    private var notifications: NotificationService?
    private var coach: FirstRunCoachController?
    private var started = false

    init() {
        let settings = SettingsStore()
        let store = JobStore()
        let tunnels = MachineTunnelManager()
        self.settings = settings
        self.store = store
        self.tunnels = tunnels
        self.hubClient = HubClient(store: store)
        self.hubProcessManager = HubProcessManager()
        store.settingsProvider = { [weak self] in
            self?.settings ?? SettingsStore()
        }
        // Panel Clear / demo are hub round-trips: the store is a cache, so a
        // local edit would be overwritten by the next frame.
        store.clearRequestSink = { [weak self] in
            self?.hubClient.clearAll()
        }
        store.demoRequestSink = { [weak self] in
            self?.hubClient.loadDemo()
        }
        store.refreshRequestSink = { [weak self] in
            self?.hubClient.refresh()
        }
        store.notifyPolicySink = { [weak self] policy in
            self?.hubClient.setNotifyPolicy(policy)
        }
        tunnels.attach(settings: settings)
    }

    func start() {
        guard !started else { return }
        started = true

        let notifications = NotificationService(settings: settings)
        self.notifications = notifications
        notifications.requestAuthorizationIfNeeded()
        notifications.onOpenJob = { [weak self] jobId in
            self?.openJobFromNotification(jobId)
        }
        // Panel deep-link helper used when a banner is clicked.
        notifications.onRevealPanel = { [weak self] in
            self?.revealStatusPanelBestEffort()
        }

        store.notificationSink = { [weak self] event in
            self?.notifications?.deliver(event)
        }

        // SwiftUI observes the store and settings directly for MenuBarExtra updates.
        store.ribbonInvalidationSink = nil
        settings.ribbonAppearanceSink = nil

        // A stream that ends is the only sign this app gets that a hub died.
        // Re-probe, and spawn again only if nothing answers — the manager holds
        // the health check and the 10s throttle that keep that from storming.
        hubClient.onStreamEnded = { [weak self] in
            guard let self else { return }
            Task { await self.hubProcessManager.ensureRunning() }
        }

        // State lives in `nerve-hub`; this app only paints its frames. Make sure
        // one is up — reusing whichever hub already owns the port — before
        // attaching, so a cold launch has something to attach to.
        Task { [weak self] in
            guard let self else { return }
            await self.hubProcessManager.ensureRunning()
            self.hubClient.connect()
        }

        tunnels.start()

        if !settings.hasSeenCoachMarks {
            let coach = FirstRunCoachController()
            self.coach = coach
            coach.show { [weak self] loadDemo in
                guard let self else { return }
                self.settings.hasSeenCoachMarks = true
                self.settings.hasCompletedFirstRun = true
                if loadDemo {
                    self.hubClient.loadDemo()
                }
            }
        } else if !settings.hasCompletedFirstRun {
            settings.hasCompletedFirstRun = true
        }
    }

    /// Banner click: select + expand the job, Open/Focus the agent workspace, try to show panel.
    func openJobFromNotification(_ jobId: String) {
        _ = store.focusAndOpenJob(id: jobId)
        revealStatusPanelBestEffort()
    }

    /// Best-effort: click the MenuBarExtra status item so the panel appears with the job expanded.
    /// MenuBarExtra has no public open API — we locate our status-bar chrome and performClick.
    private func revealStatusPanelBestEffort() {
        NSApp.activate(ignoringOtherApps: true)
        // If the panel is already up, selectedJobId + onChange expands the row.
        if store.panelOpen { return }

        // Prefer a visible status-item button owned by this process.
        for window in NSApp.windows {
            let name = NSStringFromClass(type(of: window))
            guard name.contains("StatusBar") || name.contains("StatusItem") else { continue }
            if let button = window.contentView?.subviews.compactMap({ $0 as? NSStatusBarButton }).first
                ?? window.contentView as? NSStatusBarButton {
                button.performClick(nil)
                return
            }
            // Fallback: synthesize a click at the window center (status item).
            let frame = window.frame
            let point = NSPoint(x: frame.midX, y: frame.midY)
            if let event = NSEvent.mouseEvent(
                with: .leftMouseDown,
                location: window.convertPoint(fromScreen: point),
                modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber,
                context: nil,
                eventNumber: 0,
                clickCount: 1,
                pressure: 1
            ) {
                window.sendEvent(event)
            }
            if let up = NSEvent.mouseEvent(
                with: .leftMouseUp,
                location: window.convertPoint(fromScreen: point),
                modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber,
                context: nil,
                eventNumber: 1,
                clickCount: 1,
                pressure: 1
            ) {
                window.sendEvent(up)
            }
            return
        }
    }

    /// Call after Settings mutates the machine list.
    func machinesDidChange() {
        tunnels.syncConfig()
    }

    func quit() {
        guard started else { return }
        started = false
        tunnels.stopAll()
        // Detach only — the hub outlives this app and serves other surfaces.
        hubClient.disconnect()
    }
}
