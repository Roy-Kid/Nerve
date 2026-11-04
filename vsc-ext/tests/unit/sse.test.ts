import { strict as assert } from "node:assert";
import { SseParser } from "../../src/hub/sse";

suite("sse parser", () => {

  test("ignores keep-alive comments", () => {
    const parser = new SseParser();
    assert.deepEqual(parser.push(": keep-alive\n\ndata: x\n\n"), ["x"]);
  });

  test("joins multi-line data fields", () => {
    const parser = new SseParser();
    assert.deepEqual(parser.push("data: a\ndata: b\n\n"), ["a\nb"]);
  });

  test("holds a partial event across chunks", () => {
    const parser = new SseParser();
    assert.deepEqual(parser.push("data: hel"), []);
    assert.deepEqual(parser.push("lo\n\n"), ["hello"]);
  });
});
