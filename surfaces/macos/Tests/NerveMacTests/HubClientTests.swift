import Foundation
import Testing
@testable import NerveMac

@MainActor
@Suite("Hub surface registration")
struct HubClientTests {
  @Test func nativeHostsRegisterTheirOwnNotificationSurface() throws {
    let mac = HubClient(store: JobStore())
    let tether = HubClient(store: JobStore(), surface: "tether")
    for (client, expected) in [(mac, "macos"), (tether, "tether")] {
      let url = client.streamURL
      let components = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false))
      let query = Dictionary(uniqueKeysWithValues: (components.queryItems ?? []).map { ($0.name, $0.value) })
      #expect(url.scheme == "http")
      #expect(url.host == "127.0.0.1")
      #expect(url.port == 17890)
      #expect(url.path == "/v1/stream")
      #expect(query["surface"] == expected)
      #expect(query["instanceId"] == HubClient.instanceId)
      #expect(query["notifications"] == "true")
    }
  }
}
