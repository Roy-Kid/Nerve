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

    init(settings: SettingsStore) {
        self.settings = settings
        super.init()
        center.delegate = self
    }

    func requestAuthorizationIfNeeded() {
        center.getNotificationSettings { [weak self] settings in
            let authorizationStatus = settings.authorizationStatus
            Task { @MainActor [weak self] in
                guard let self else { return }
                switch authorizationStatus {
                case .notDetermined:
                    do {
                        let granted = try await self.center.requestAuthorization(options: [.alert, .sound, .badge])
                        if !granted {
                            NerveLog.notify.error("notification auth: user denied")
                        }
                    } catch {
                        NerveLog.notify.error("notification auth: \(String(describing: error), privacy: .public)")
                    }
                case .denied:
                    NerveLog.notify.error("notifications denied — enable in System Settings → Notifications → Nerve")
                default:
                    break
                }
            }
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
            var request = URLRequest(url:URL(string:"http://127.0.0.1:17890/v1/notifications/result")!)
            request.httpMethod = "POST"
            request.setValue("application/json",forHTTPHeaderField:"Content-Type")
            request.httpBody = try? JSONSerialization.data(withJSONObject:["id":event.id,"delivered":error == nil])
            URLSession.shared.dataTask(with:request).resume()
        }
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
