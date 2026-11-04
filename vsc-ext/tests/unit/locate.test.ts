import { strict as assert } from "node:assert";
import { join } from "node:path";
import {
  HUB_BINARY,
  locateHub,
  platformFixedDirectories,
  type BinaryProbe,
} from "../../src/hub/locate";

/**
 * The rule under test is the order. It is the same on every platform, so the
 * directory list is injected and the expected paths are built with `join` —
 * a literal would encode one OS's separator and fail on the other.
 */
const INSTALL = ["/install/first", "/install/second"];

const first = join(INSTALL[0], HUB_BINARY);
const second = join(INSTALL[1], HUB_BINARY);
const cargo = join("/home/me", ".cargo", "bin", HUB_BINARY);

function probe(hits: Record<string, boolean>, pathHit?: string): BinaryProbe {
  return {
    isExecutable(path: string) {
      return Boolean(hits[path]);
    },
    onPath() {
      return pathHit;
    },
  };
}

function locate(hits: Record<string, boolean>, pathHit?: string) {
  return locateHub(probe(hits, pathHit), "/home/me", undefined, INSTALL);
}

suite("hub locate", () => {
  test("prefers a checkout release binary", () => {
    const checkout = join("/repo/vsc-ext", "..", "target", "release", HUB_BINARY);
    const found = locateHub(
      probe({ [checkout]: true }),
      "/home/me",
      "/repo/vsc-ext",
      INSTALL,
    );
    assert.equal(found, checkout);
  });

  test("an install directory beats cargo", () => {
    assert.equal(locate({ [second]: true, [cargo]: true }), second);
  });

  test("falls through to PATH", () => {
    assert.equal(locate({}, "/usr/bin/nerve-hub"), "/usr/bin/nerve-hub");
  });

  test("windows install directories come from the environment", () => {
    const dirs = platformFixedDirectories({
      LOCALAPPDATA: "C:\\Users\\me\\AppData\\Local",
      ProgramFiles: "C:\\Program Files",
    });
    if (process.platform !== "win32") {
      // Off Windows the function answers for this host, not for that env.
      assert.deepEqual(dirs, ["/opt/homebrew/bin", "/usr/local/bin"]);
      return;
    }
    assert.deepEqual(dirs, [
      join("C:\\Users\\me\\AppData\\Local", "Programs", "Nerve"),
      join("C:\\Program Files", "Programs", "Nerve"),
    ]);
  });
});
