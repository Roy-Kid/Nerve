import SwiftUI
import TetherPluginKit
#if os(macOS)
import NerveMac
#endif

/// Tether's Nerve lamp. The panel, ribbon and settings window are the macOS
/// app's views; this type only starts them.
@MainActor
public final class NervePlugin: TetherPlugin {
  public let metadata = PluginMetadata(
    id: "app.nerve.tether",
    name: "Nerve",
    symbol: "waveform.path.ecg",
    summary: "Agent status"
  )
  public var isStatusBarOnly: Bool { true }
  public var needsRemoteConnection: Bool { false }
  public var statusBarItem: PluginStatusBarItem? {
    PluginStatusBarItem(id: metadata.id, label: metadata.name, segments: [])
  }

  #if os(macOS)
  private let model = AppModel()
  #endif

  public init() {}

  public func activate() {
    #if os(macOS)
    model.start()
    #endif
  }

  public func deactivate() {
    #if os(macOS)
    model.quit()
    #endif
  }

  public func launch(in context: PluginContext) {}

  public func statusBarLabel() -> AnyView? {
    #if os(macOS)
    AnyView(MenuBarRibbonLabel(store: model.store, settings: model.settings, compact: true))
    #else
    nil
    #endif
  }

  public func statusBarSettings() -> (() -> Void)? {
    #if os(macOS)
    let model = model
    return {
      PreferencesWindowController.show(
        store: model.store, settings: model.settings, tunnels: model.tunnels)
    }
    #else
    nil
    #endif
  }

  public func statusBarWorkspace() -> (any PluginWorkspace)? {
    #if os(macOS)
    NerveHostedPanel(model: model)
    #else
    nil
    #endif
  }

  public func settings() -> AnyView {
    #if os(macOS)
    let model = model
    return AnyView(Button {
      PreferencesWindowController.show(
        store: model.store, settings: model.settings, tunnels: model.tunnels)
    } label: {
      Label("Open Nerve Settings", systemImage: "gearshape")
    }
    .help("Opens the Nerve settings window"))
    #else
    AnyView(EmptyView())
    #endif
  }
}

#if os(macOS)
@MainActor
private final class NerveHostedPanel: PluginWorkspace {
  let id = UUID()
  let title = "Nerve"
  let subtitle = ""
  let symbol = "waveform.path.ecg"
  let commands: [PluginCommand] = []
  private let model: AppModel

  init(model: AppModel) { self.model = model }

  func content() -> AnyView {
    AnyView(StatusPanelRoot(store: model.store, settings: model.settings))
  }

  func inspector() -> AnyView { AnyView(EmptyView()) }

  func close() {}
}
#endif
