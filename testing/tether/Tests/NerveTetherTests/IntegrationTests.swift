import AppKit
import Foundation
import Testing
import WebKit
import TetherPluginKit
@testable import TetherPluginHost

@MainActor private final class Consent: ConsentAsking {
  func ask(pluginName: String, permission: Permission) async -> Bool { true }
}
@MainActor private final class DemoStream: PluginStreaming {
  var connections = 0
  var closed = 0
  let json: String
  var receivers: [UUID: @MainActor (JSONValue) async -> Void] = [:]
  init(json: String) { self.json = json }
  func stream(url: URL, receive: @escaping @MainActor (JSONValue) async -> Void) async throws {
    connections += 1
    let id = UUID()
    receivers[id] = receive
    defer { closed += 1; receivers[id] = nil }
    await receive(.object(["event": .string("message"), "data": .string(json)]))
    try await Task.sleep(for: .seconds(30))
  }

  func send(_ frame: [String: Any]) async throws {
    let data = try JSONSerialization.data(withJSONObject: frame)
    let event = JSONValue.object(["event": .string("message"), "data": .string(String(decoding: data, as: UTF8.self))])
    for receive in receivers.values { await receive(event) }
  }
}

@MainActor @Suite("Nerve for Tether", .serialized)
struct IntegrationTests {
  private var surfaces: URL {
    URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
      .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
      .appending(path: "surfaces")
  }

  @Test func backgroundRibbonAndPanelUseRealPluginBytes() async throws {
    _ = NSApplication.shared
    let snapshot = try Data(contentsOf: surfaces.deletingLastPathComponent().appending(path: "fixtures/website-frame.json"))
    var frame = try #require(JSONSerialization.jsonObject(with: snapshot) as? [String: Any])
    frame["departed"] = []
    frame["tabIndicators"] = [["target": ["host": "studio", "terminal": "/dev/ttys001"],
      "status": "attention", "label": "Approval needed"]]
    var jobs = try #require(frame["jobs"] as? [[String: Any]])
    jobs[0]["timeline"] = [["timestamp": "2026-10-18T12:00:00Z", "title": "Build failed"]]
    frame["jobs"] = jobs
    let json = String(decoding: try JSONSerialization.data(withJSONObject: frame), as: UTF8.self)
    let transport = DemoStream(json: json)
    let host = PluginHost(consent: Consent(), streaming: transport)
    let record = try host.install(directory: surfaces.appending(path: "tether"))
    host.grantNetwork(pluginID: record.id, urls: record.manifest.networkURLs)
    #expect(record.manifest.contributions.map(\.kind) == [.panel, .statusBar, .tabIndicator])
    var tabMarks: [UUID: [PluginTabIndicator]] = [:]
    host.onTabIndicators = { session, _, items in tabMarks[session] = items }
    var label: String?
    host.onStatus = { id, payload in
      #expect(id == "app.nerve.tether")
      label = payload.key("label")?.string
    }
    let background = WebKitRuntime(background: true)
    let backgroundSession = try await host.activate(id: record.id, runtime: background)
    defer { host.unload(session: backgroundSession) }
    for _ in 0..<500 where label?.contains(" jobs") != true { try await Task.sleep(for: .milliseconds(10)) }
    #expect(label?.contains("jobs") == true)
    #expect(label?.contains("need you") == true)
    for _ in 0..<100 where tabMarks[backgroundSession]?.isEmpty != false { try await Task.sleep(for: .milliseconds(10)) }
    #expect(tabMarks[backgroundSession]?.first?.color == 0xFF9F0A)
    let panel = WebKitRuntime()
    let panelSession = try await host.activate(id: record.id, runtime: panel)
    defer { host.unload(session: panelSession) }
    let page = try #require(panel.webView(for: panelSession))
    let window = NSWindow(contentRect: NSRect(x: -4000, y: -4000, width: 400, height: 450),
                          styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = page
    window.orderFrontRegardless()
    defer { window.orderOut(nil) }
    var rows = 0
    for _ in 0..<100 where rows == 0 {
      try await Task.sleep(for: .milliseconds(10))
      rows = (try? await page.evaluateJavaScript("document.querySelectorAll('.row').length") as? Int) ?? 0
    }
    #expect(rows > 0)
    #expect(transport.connections == 2)
    #expect(tabMarks[panelSession] == nil, "only the background session publishes tab marks")
    let content = try await page.evaluateJavaScript("document.body.textContent") as? String
    #expect(content?.contains("deploy edge") == true)
    #expect(content?.contains("Codex · refactor") == true)
    let connected = try await page.evaluateJavaScript("document.getElementById('connection').title") as? String
    #expect(connected == "Connected")
    let symbols = try await page.evaluateJavaScript("document.querySelectorAll('[data-filter] .dot').length") as? Int
    #expect(symbols == 8)
    let visibleLabels = try await page.evaluateJavaScript("Array.from(document.querySelectorAll('[data-filter]')).every(button => button.textContent === '' && button.hasAttribute('aria-label'))") as? Bool
    #expect(visibleLabels == true)
    let statusWords = try await page.evaluateJavaScript("document.querySelectorAll('.state').length") as? Int
    #expect(statusWords == 0)
    _ = try await page.evaluateJavaScript("document.querySelector('[data-filter=problem]').click()")
    let filteredRows = try await page.evaluateJavaScript("document.querySelectorAll('.row').length") as? Int
    #expect(filteredRows == 1)
    _ = try await page.evaluateJavaScript("document.querySelector('[data-filter=all]').click()")
    _ = try await page.evaluateJavaScript("document.querySelector('[data-filter=monitor]').click()")
    let monitor = try await page.evaluateJavaScript("document.querySelector('.row')?.dataset.jobId") as? String
    #expect(monitor == "stream")
    _ = try await page.evaluateJavaScript("document.querySelector('[data-filter=all]').click(); document.getElementById('search-toggle').click(); document.getElementById('search').value = 'CODEX'; document.getElementById('search').dispatchEvent(new Event('input'))")
    let searchRows = try await page.evaluateJavaScript("document.querySelectorAll('.row').length") as? Int
    #expect(searchRows == 1)
    _ = try await page.evaluateJavaScript("document.getElementById('search').value = ''; document.getElementById('search').dispatchEvent(new Event('input')); document.getElementById('search-toggle').click(); document.querySelector('summary').click(); document.querySelector('summary').focus(); window.heldRow = document.querySelector('.row'); void 0")
    frame["revision"] = 2
    try await transport.send(frame)
    let stable = try await page.evaluateJavaScript("window.heldRow === document.querySelector('.row') && window.heldRow.open && document.activeElement === window.heldRow.firstElementChild") as? Bool
    #expect(stable == true)
    let timeline = try await page.evaluateJavaScript("document.querySelector('.timeline')?.textContent.includes('Build failed')") as? Bool
    #expect(timeline == true)
    var malformed = frame
    malformed["revision"] = 100
    malformed["jobs"] = [["id": "broken"]]
    try await transport.send(malformed)
    let invalid = try await page.evaluateJavaScript("document.body.dataset.connection") as? String
    #expect(invalid == "invalid")
    let retained = try await page.evaluateJavaScript("document.querySelectorAll('.row').length") as? Int
    #expect(retained == rows)
    frame["revision"] = 3
    try await transport.send(frame)
    let recovered = try await page.evaluateJavaScript("document.body.dataset.connection") as? String
    #expect(recovered == "live", "a bad high-revision snapshot cannot poison the revision")
    var older = frame
    older["revision"] = 1
    older["jobs"] = []
    try await transport.send(older)
    let ignored = try await page.evaluateJavaScript("document.querySelectorAll('.row').length") as? Int
    #expect(ignored == rows, "older revisions from the same hub cannot erase current rows")
    _ = try await page.evaluateJavaScript("document.querySelector('summary').dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowDown', bubbles:true}))")
    let next = try await page.evaluateJavaScript("document.activeElement?.parentElement.dataset.jobId") as? String
    #expect(next == "codex")
    _ = try await page.evaluateJavaScript("tether._event({type:'end'})")
    let stale = try await page.evaluateJavaScript("document.getElementById('jobs').classList.contains('stale') && document.querySelectorAll('.row').length > 0") as? Bool
    #expect(stale == true)
    frame["revision"] = 4
    try await transport.send(frame)
    _ = try await page.evaluateJavaScript("document.querySelector('.row').open = false; document.activeElement.blur()")
    if let output = ProcessInfo.processInfo.environment["NERVE_PLUGIN_SNAPSHOT"] {
      let image = try await page.takeSnapshot(configuration: nil)
      let data = try #require(image.tiffRepresentation)
      let bitmap = try #require(NSBitmapImageRep(data: data))
      try #require(bitmap.representation(using: .png, properties: [:])).write(to: URL(fileURLWithPath: output))
    }
    var restart = frame
    restart["hubInstanceId"] = "new-hub"
    restart["revision"] = 0
    restart["jobs"] = []
    restart["machines"] = [["id": "remote", "host": "gpu-box", "state": "offline"]]
    try await transport.send(restart)
    let replaced = try await page.evaluateJavaScript("document.querySelectorAll('.row').length === 0 && document.querySelector('.machine')?.textContent === 'gpu-box'") as? Bool
    #expect(replaced == true, "a new hub replaces its snapshot, including offline machines without jobs")
    _ = try await background.webView(for: backgroundSession)?.evaluateJavaScript("tether._event({type:'end'})")
    for _ in 0..<100 where tabMarks[backgroundSession]?.isEmpty != true { try await Task.sleep(for: .milliseconds(10)) }
    #expect(tabMarks[backgroundSession] == [], "a disconnected stream cannot leave a live tab mark")
    _ = try await page.evaluateJavaScript("tether._event({type:'end'}); document.getElementById('retry').click()")
    for _ in 0..<100 where transport.connections != 3 || transport.closed < 1 {
      try await Task.sleep(for: .milliseconds(10))
    }
    #expect(transport.connections == 3)
    #expect(transport.closed == 1, "manual reconnect closes the old stream before opening another")
    try host.setEnabled(false, id: record.id)
    for _ in 0..<20 where transport.closed != 3 { await Task.yield() }
    #expect(host.liveSessions.isEmpty)
    #expect(transport.closed == 3)
    #expect(panel.webView(for: panelSession) == nil)
    #expect(background.webView(for: backgroundSession) == nil)
  }
}

extension IntegrationTests {
  @Test func missingNetworkGrantIsExplainedWithoutStartingATransport() async throws {
    let transport = DemoStream(json: "{}")
    let host = PluginHost(consent: Consent(), streaming: transport)
    let record = try host.install(directory: surfaces.appending(path: "tether"))
    let runtime = WebKitRuntime()
    let session = try await host.activate(id: record.id, runtime: runtime)
    defer { host.unload(session: session) }
    let page = try #require(runtime.webView(for: session))
    var phase: String?
    for _ in 0..<100 where phase != "denied" {
      try await Task.sleep(for: .milliseconds(10))
      phase = try await page.evaluateJavaScript("document.body.dataset.connection") as? String
    }
    #expect(phase == "denied")
    #expect(transport.connections == 0)
    let recoverable = try await page.evaluateJavaScript("!document.getElementById('retry').hidden && document.getElementById('connection').title.includes('Settings')") as? Bool
    #expect(recoverable == true)
  }

  @Test(.enabled(if: ProcessInfo.processInfo.environment["NERVE_PLUGIN_LIVE_HUB"] == "1"))
  func liveHubUpdatesBackgroundRibbon() async throws {
    let host = PluginHost(consent: Consent())
    let record = try host.install(directory: surfaces.appending(path: "tether"))
    host.grantNetwork(pluginID: record.id, urls: record.manifest.networkURLs)
    var label: String?
    host.onStatus = { _, payload in label = payload.key("label")?.string }
    let runtime = WebKitRuntime(background: true)
    let session = try await host.activate(id: record.id, runtime: runtime)
    defer { host.unload(session: session) }
    for _ in 0..<200 where label?.contains(" jobs") != true {
      try await Task.sleep(for: .milliseconds(10))
    }
    #expect(label?.contains(" jobs") == true, "real URLSession SSE reaches the actual plugin")
    #expect(label?.contains("Disconnected") != true)
  }
}
