import SwiftUI
import AppKit

@main
struct NerveApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var menuBarItemEnabled = true

    var body: some Scene {
        // MenuBarExtra will not re-evaluate the label unless this Scene
        // itself reads store state. `revision` is the write counter.
        let _ = appDelegate.model.store.revision
        let _ = appDelegate.model.store.activeCount
        MenuBarExtra(isInserted: menuBarInsertion) {
            StatusPanelRoot(
                store: appDelegate.model.store,
                settings: appDelegate.model.settings
            )
        } label: {
            MenuBarRibbonLabel(
                store: appDelegate.model.store,
                settings: appDelegate.model.settings
            )
            // Right-click menu is installed via AppKit
            // (`MenuBarExtraContextMenuBridge`) — SwiftUI `.contextMenu` on the
            // label is unreliable with `.menuBarExtraStyle(.window)`.
            .help("Nerve — click for status, right-click for Settings")
        }
        .menuBarExtraStyle(.window)

        Settings {
            PreferencesView(
                store: appDelegate.model.store,
                settings: appDelegate.model.settings,
                tunnels: appDelegate.model.tunnels
            )
        }
    }

    private var menuBarInsertion: Binding<Bool> {
        Binding(
            get: {
                menuBarItemEnabled
                    && (!appDelegate.model.settings.hideWhenIdle
                        || appDelegate.model.store.activeCount > 0)
            },
            set: { menuBarItemEnabled = $0 }
        )
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = AppModel()
    private var ribbonContextMenu: MenuBarExtraContextMenuBridge?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // `nil` is the AppKit contract for inheriting the current macOS appearance.
        // Keep this app system-driven; individual windows and popovers inherit it.
        NSApp.appearance = nil
        NSApp.setActivationPolicy(.accessory)
        model.start()

        // MenuBarExtra(.window) does not honor label context menus — install AppKit bridge.
        let bridge = MenuBarExtraContextMenuBridge { [weak self] in
            self?.showSettings()
        }
        bridge.start()
        ribbonContextMenu = bridge
        NSAppleEventManager.shared().setEventHandler(
            self,
            andSelector: #selector(handleGetURL(_:withReplyEvent:)),
            forEventClass: AEEventClass(kInternetEventClass),
            andEventID: AEEventID(kAEGetURL)
        )
    }

    /// `nerve://settings` from Tether. Same window as the ribbon's Settings….
    func application(_ application: NSApplication, open urls: [URL]) {
        guard urls.contains(where: Self.isSettingsURL) else { return }
        showSettings()
    }

    func showSettings() {
        PreferencesWindowController.show(
            store: model.store,
            settings: model.settings,
            tunnels: model.tunnels
        )
    }

    @objc private func handleGetURL(_ event: NSAppleEventDescriptor, withReplyEvent reply: NSAppleEventDescriptor) {
        guard let text = event.paramDescriptor(forKeyword: keyDirectObject)?.stringValue,
              let url = URL(string: text), Self.isSettingsURL(url) else { return }
        showSettings()
    }

    private static func isSettingsURL(_ url: URL) -> Bool {
        url.scheme?.caseInsensitiveCompare("nerve") == .orderedSame
            && url.host?.caseInsensitiveCompare("settings") == .orderedSame
            && (url.path.isEmpty || url.path == "/")
            && url.query == nil
    }

    func applicationWillTerminate(_ notification: Notification) {
        ribbonContextMenu?.stop()
        ribbonContextMenu = nil
        model.quit()
    }
}
