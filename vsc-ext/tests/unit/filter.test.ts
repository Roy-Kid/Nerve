import { project } from "../project";
import { strict as assert } from "node:assert";
import { jobInFolder, matchesFilter } from "../../src/model/filter";
import { parseJob, type Job } from "../../src/model/job";

function job(value: Record<string, unknown>): Job {
  const parsed = parseJob(project({ id: "a", ...value }));
  if (!parsed) throw new Error("parseJob rejected a test fixture");
  return parsed;
}

suite("filter", () => {
  test("this folder matches workspace prefix", () => {
    const row = job({ context: { workspace: "/Users/me/work/nerve" } });
    assert.equal(jobInFolder(row, ["/Users/me/work/nerve"]), true);
    assert.equal(jobInFolder(row, ["/Users/me/work/other"]), false);
    assert.equal(
      jobInFolder(
        job({ context: { workspace: "/Users/me/work/nerve-extra" } }),
        ["/Users/me/work/nerve"],
      ),
      false,
    );
  });

  test("this folder matches a windows workspace too", () => {
    // The folder filter was dead on Windows: the comparison only knew `/`.
    const row = job({ context: { workspace: "C:\\Users\\me\\work\\nerve" } });
    assert.equal(jobInFolder(row, ["C:\\Users\\me\\work"]), true);
    assert.equal(jobInFolder(row, ["C:/Users/me/work"]), true);
    assert.equal(jobInFolder(row, ["C:\\Users\\me\\other"]), false);
  });

  test("attention filter is needs-you, not a failure", () => {
    assert.equal(
      matchesFilter(
        job({ lifecycle: "active", outcome: "failure" }),
        "attention",
        [],
      ),
      false,
    );
    assert.equal(
      matchesFilter(
        job({
          lifecycle: "active",
          attention: { level: "required", reason: "approval" },
        }),
        "attention",
        [],
      ),
      true,
    );
    assert.equal(
      matchesFilter(job({ lifecycle: "active" }), "attention", []),
      false,
    );
    assert.equal(
      matchesFilter(
        job({ lifecycle: "active", current: { type: "monitor" } }),
        "running",
        [],
      ),
      false,
    );
  });

  test("all is unfiltered", () => {
    assert.equal(matchesFilter(job({ lifecycle: "active" }), "all", []), true);
  });
});
