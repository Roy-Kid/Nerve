import Testing
@testable import NerveMac

@Suite("Event-stream framing")
struct SSEDecoderTests {
  private func events(_ wire: String) -> [String] {
    var decoder = SSEDecoder()
    return wire.utf8.compactMap { decoder.feed($0) }
  }

  @Test func aBlankLineEndsEachEvent() {
    #expect(events("data: {\"a\":1}\n\ndata: {\"b\":2}\n\n") == ["{\"a\":1}", "{\"b\":2}"])
  }

  @Test func dataLinesJoinAndOtherFieldsAndCommentsAreSkipped() {
    #expect(events(": keep-alive\n\nevent: frame\nid: 7\ndata: one\ndata:two\n\n") == ["one\ntwo"])
  }

  @Test func carriageReturnLineEndsAreFraming() {
    #expect(events("data: x\r\n\r\n") == ["x"])
  }

  @Test func anEventWaitsForItsBlankLine() {
    #expect(events("data: half\n").isEmpty)
  }

  @Test func multibyteTextSurvivesByteFeeding() {
    #expect(events("data: 轮到你\n\n") == ["轮到你"])
  }
}
