import AppKit
import SwiftUI

// MARK: - Window

@MainActor
public final class PreferencesWindowController: NSWindowController {
    private static var shared: PreferencesWindowController?

    public static func show(
        store: JobStore,
        settings: SettingsStore,
        tunnels: MachineTunnelManager,
        onRibbonRefresh: (() -> Void)? = nil
    ) {
        if let existing = shared {
            existing.window?.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let root = PreferencesView(
            store: store,
            settings: settings,
            tunnels: tunnels,
            onRibbonRefresh: onRibbonRefresh
        )
        let hosting = NSHostingController(rootView: root)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 700, height: 540),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Nerve Settings"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.toolbarStyle = .unified
        window.titlebarSeparatorStyle = .none
        window.tabbingMode = .disallowed
        window.appearance = nil
        window.contentViewController = hosting
        window.minSize = NSSize(width: 660, height: 500)
        window.setContentSize(NSSize(width: 700, height: 540))
        window.center()
        window.isReleasedWhenClosed = false

        let controller = PreferencesWindowController(window: window)
        shared = controller
        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { _ in
            Task { @MainActor in
                shared = nil
            }
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}
