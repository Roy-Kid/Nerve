import Foundation
import UserNotifications
import AppKit

/// Executes explicit hub notification commands and reports OS delivery.
@MainActor
final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    private let settings: SettingsStore
    private let center = UNUserNotificationCenter.current()

    /// Select + Open the job (AppModel wires focusAndOpenJob).
    var onOpenJob: ((String) -> Void)?
    /// Best-effort show the status panel after a banner click.
    var onRevealPanel: (() -> Void)?
    /// Authorization is known, including after the system prompt. The hub
    /// client pushes eligibility again so an Ask is not stuck behind the
    /// first "not determined" answer.
    var onAuthorizationSettled: (() -> Void)?
    /// Runs after the launch prompt, on the main actor.
    var onPermissionPromptFinished: (() -> Void)?

    init(settings: SettingsStore) {
        self.settings = settings
        super.init()
        center.delegate = self
    }

    /// First launch asks for permission. A menu-bar app only gets the system
    /// dialog while it is briefly a regular app. After a denial, macOS will
    /// not ask again, so the prompt opens System Settings instead.
    func explainPermissionIfNeeded() {
        center.getNotificationSettings { [weak self] settings in
            let authorizationStatus = settings.authorizationStatus
            Task { @MainActor [weak self] in
                guard let self else { return }
                await self.explain(authorizationStatus)
                self.onPermissionPromptFinished?()
            }
        }
    }

    private func explain(_ authorizationStatus: UNAuthorizationStatus) async {
        NerveLog.record("notification auth status=\(authorizationStatus.rawValue)")
        if authorizationStatus == .notDetermined {
            await requestSystemPrompt()
            settings.hasExplainedNotificationPermission = true
            onAuthorizationSettled?()
            return
        }
        if authorizationStatus == .denied, !settings.hasExplainedNotificationPermission {
            NerveLog.record("notification auth denied")
            offerSystemSettings()
            settings.hasExplainedNotificationPermission = true
        }
        onAuthorizationSettled?()
    }

    private func requestSystemPrompt() async {
        let previous = NSApp.activationPolicy()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        defer { NSApp.setActivationPolicy(previous) }
        do {
            let granted = try await center.requestAuthorization(options: [.alert, .sound, .badge])
            NerveLog.record("notification auth granted=\(granted)")
            if !granted {
                NerveLog.notify.error("notification auth: user denied")
            }
        } catch {
            NerveLog.record("notification auth error=\(error.localizedDescription)")
            NerveLog.notify.error("notification auth: \(String(describing: error), privacy: .public)")
        }
    }

    private func offerSystemSettings() {
        let previous = NSApp.activationPolicy()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        defer { NSApp.setActivationPolicy(previous) }
        let alert = NSAlert()
        alert.messageText = "Allow notifications"
        alert.informativeText = "Nerve banners you when an agent needs you, fails, or finishes a long task. Turn on notifications for Nerve in System Settings."
        alert.addButton(withTitle: "Open System Settings")
        alert.addButton(withTitle: "Not Now")
        if alert.runModal() == .alertFirstButtonReturn {
            Self.openNotificationSettings()
        }
    }

    static func openNotificationSettings() {
        let bundle = Bundle.main.bundleIdentifier ?? "app.nerve.Nerve"
        let candidates = [
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=\(bundle)",
            "x-apple.systempreferences:com.apple.preference.notifications?id=\(bundle)",
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
        ]
        for raw in candidates {
            guard let url = URL(string: raw), NSWorkspace.shared.open(url) else { continue }
            return
        }
    }

    func deliver(_ event: HubNotification) {
        let content = UNMutableNotificationContent()
        content.title = event.title
        content.body = event.body.isEmpty ? event.title : event.body
        content.sound = event.sound ? .default : nil
        content.userInfo = ["subjectId":event.jobId]
        content.threadIdentifier = event.jobId
        center.add(UNNotificationRequest(identifier:event.id,content:content,trigger:nil)) { error in
            Self.report(event.id, delivered: error == nil)
        }
    }

    /// Tell the hub how executing one of its notifications ended.
    nonisolated static func report(_ id: String, delivered: Bool) {
        var request = URLRequest(url:URL(string:"http://127.0.0.1:17890/v1/notifications/result")!)
        request.httpMethod = "POST"
        request.setValue("application/json",forHTTPHeaderField:"Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject:["id":id,"delivered":delivered])
        URLSession.shared.dataTask(with:request).resume()
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let info = response.notification.request.content.userInfo
        let subjectId = info["subjectId"] as? String
        Task { @MainActor in
            if let subjectId {
                self.onOpenJob?(subjectId)
            } else {
                self.onRevealPanel?()
            }
        }
        completionHandler()
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        // Menu-bar apps stay "foreground"; still show banners.
        completionHandler([.banner, .sound, .list])
    }
}
