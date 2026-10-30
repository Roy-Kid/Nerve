import AppKit
import SwiftUI
import Testing
@testable import NerveMac

@MainActor
@Suite("Hosted row clicks", .serialized)
struct HostedFocusTests {
  @Test func aHostLandingReplacesTheFolder() {
    var seen: PanelFocusBridge.Request?
    PanelFocusBridge.land = { request in
      seen = request
      return true
    }
    defer { PanelFocusBridge.land = nil }

    var job = Job.make(
      id: "1", name: "Build", alias: LocalMachine.alias, producer: ProducerInfo(id: "claude"))
    job.extensions["terminal"] = .object([
      "host": .string("macbook.local"),
      "terminal": .string("/dev/ttys004"),
    ])
    job.context = ContextInfo(project: nil, workspace: "/tmp/proj", labels: nil)
    job.location = LocationInfo(openURL: "file:///tmp/proj", focusHint: "/tmp/proj", logPath: nil)

    let result = ActionService.openLocation(job)
    #expect(seen?.host == "macbook.local")
    #expect(seen?.terminal == "/dev/ttys004")
    #expect(seen?.directory == "/tmp/proj")
    #expect(seen?.url == "file:///tmp/proj")
    guard case .succeeded(let message) = result else {
      Issue.record("a hosted click stays with the host")
      return
    }
    #expect(message.isEmpty)
  }

  @Test func aRemoteWorkspaceNeverReachesTheLocalHostLanding() {
    var landed = false
    PanelFocusBridge.land = { _ in
      landed = true
      return true
    }
    defer { PanelFocusBridge.land = nil }

    var job = Job.make(
      id: "remote", name: "Build", alias: LocalMachine.alias + "-remote",
      producer: ProducerInfo(id: "claude"))
    job.context = ContextInfo(project: nil, workspace: "/tmp/proj", labels: nil)
    // No remote URL or SSH host: the existing remote path returns failure
    // instead of matching a local tab that happens to use /tmp/proj.
    let result = ActionService.openLocation(job)
    #expect(!landed)
    guard case .failed = result else {
      Issue.record("an unmatched remote workspace must not report local success")
      return
    }
  }

  @Test func anUnmatchedHostLandingDoesNotReportSuccess() {
    PanelFocusBridge.land = { _ in false }
    defer { PanelFocusBridge.land = nil }
    let job = Job.make(
      id: "local", name: "Build", alias: LocalMachine.alias,
      producer: ProducerInfo(id: "claude"))
    guard case .failed = ActionService.openLocation(job) else {
      Issue.record("the host did not find a terminal or workspace")
      return
    }
  }

  @Test func terminalFocusDeclaresItsAutomationPurpose() throws {
    let root = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent().deletingLastPathComponent()
      .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appending(path: "Nerve/Nerve/Info.plist"))
    let plist = try #require(
      PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any])
    let purpose = plist["NSAppleEventsUsageDescription"] as? String
    #expect(purpose?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false)
  }

  @Test func oneRowFitsItsContent() {
    var job = Job.make(
      id: "1", name: "Tether", alias: "mac", producer: ProducerInfo(id: "claude"))
    job.display = HubDisplay(
      machineGroup: "mac", machineLabel: "mac", sourceOnline: true,
      status: "running", statusRank: 1, ask: false, busy: false,
      role: "job", groupId: nil, paintsRibbon: true, section: "active",
      statusLabel: "Running", activity: "Finished grep", order: 0)
    let store = JobStore()
    store.applyFrame(jobs: [job], timelines: [:])
    let root = StatusPanelRoot(store: store, settings: SettingsStore(), fitsContent: true)
    let hosting = NSHostingView(rootView: root)
    hosting.frame.size = NSSize(width: 460, height: 720)
    let fitted = hosting.fittingSize
    #expect(fitted.height > 48)
    #expect(fitted.height < 160)
  }
}
