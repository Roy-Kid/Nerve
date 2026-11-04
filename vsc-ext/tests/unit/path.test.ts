import { strict as assert } from "node:assert";
import {
  isAbsolutePath,
  isInside,
  pathFromFileUri,
  pathStyle,
} from "../../src/model/path";

suite("wire paths", () => {

  test("a drive-relative path is not absolute", () => {
    // `C:work` names a directory relative to the current one on drive C.
    assert.equal(pathStyle("C:work"), undefined);
    assert.equal(isAbsolutePath("C:work"), false);
  });

});

suite("folder containment", () => {

  test("a sibling with a shared prefix is not inside", () => {
    assert.equal(isInside("/Users/mexican", "/Users/me"), false);
  });

  test("windows separators are interchangeable", () => {
    // The folder filter was dead on Windows: `C:\Users\me\proj` never matched
    // `C:\Users\me`, because the comparison only knew about `/`.
    assert.equal(isInside("C:\\Users\\me\\proj", "C:\\Users\\me"), true);
    assert.equal(isInside("C:/Users/me/proj", "C:\\Users\\me"), true);
  });

});

suite("file uris", () => {

  test("percent escapes decode", () => {
    assert.equal(
      pathFromFileUri("file:///Users/me/proj%20foo"),
      "/Users/me/proj foo",
    );
    assert.equal(
      pathFromFileUri("file:///Users/me/%E9%A1%B9%E7%9B%AE"),
      "/Users/me/项目",
    );
  });

  test("a bad escape degrades instead of throwing", () => {
    assert.equal(pathFromFileUri("file:///tmp/100%/done"), "/tmp/100%/done");
  });

  test("a drive survives every slash count and an escaped colon", () => {
    // `file:///C:/x` used to decode to `/C:/x` — a path that exists nowhere.
    for (const uri of [
      "file:///C:/work",
      "file://C:/work",
      "file:///C%3A/work",
    ]) {
      assert.equal(pathFromFileUri(uri), "C:/work", uri);
    }
  });

});
