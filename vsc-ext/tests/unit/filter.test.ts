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

});
