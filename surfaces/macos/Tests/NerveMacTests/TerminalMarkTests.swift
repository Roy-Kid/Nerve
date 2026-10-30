import Foundation
import Testing
@testable import NerveMac

@Suite("Terminal marks")
struct TerminalMarkTests {
  /// The hub selects one status per terminal; the surface keeps its order
  /// and colours it, and selects nothing of its own.
  @Test func marksAreTheHubsInItsOrder() throws {
    let json = """
    {"protocolVersion":2,"hubInstanceId":"hub","revision":3,"jobs":[],"departed":[],
     "tabIndicators":[
       {"target":{"host":"studio","terminal":"/dev/ttys005"},"status":"attention","label":"Fix · Needs you"},
       {"target":{"host":"studio","terminal":"/dev/ttys004"},"status":"running","label":"Build · Running"},
       {"target":{"host":"studio","terminal":"/dev/ttys006"},"status":"someday","label":"Unknown"},
       {"target":{"host":"studio"},"status":"running","label":"No tty"}
     ]}
    """
    let frame = try JSONDecoder().decode(HubFrame.self, from: Data(json.utf8))
    #expect(frame.tabIndicators.count == 3, "an unreadable item drops alone")
    let colors = StatusColorMap.default
    let marks = TerminalMark.marks(from: frame.tabIndicators, colors: colors)
    #expect(marks.map(\.terminal) == ["/dev/ttys005", "/dev/ttys004"])
    #expect(marks[0].label == "Fix · Needs you")
    #expect(marks[0].color == colors.color(for: .attention).hex)
  }

  @Test func aFrameWithoutMarksHasNone() throws {
    let json = #"{"protocolVersion":2,"hubInstanceId":"hub","revision":1,"jobs":[]}"#
    let frame = try JSONDecoder().decode(HubFrame.self, from: Data(json.utf8))
    #expect(frame.tabIndicators.isEmpty)
  }

  @Test func coloursAreRibbonBytes() {
    #expect(RGBColor(r: 1, g: 0.5, b: 0).hex == 0xFF8000)
  }
}
