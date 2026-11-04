import { strict as assert } from "node:assert";
import { parseFrame, parseJobsList, lastPrompt, jobPid } from "../../src/model/job";

suite("frame", () => {
  test("decodes jobs and departed", () => {
    const frame = parseFrame(
      JSON.stringify({ protocolVersion:2,
        jobs: [{ id: "a", name: "nerve", lifecycle: "active" }],
        departed: [{ id: "b", lifecycle: "ended", outcome: "success" }],
      }),
    );
    assert.equal(frame.jobs.length, 1);
    assert.equal(frame.jobs[0]?.id, "a");
    assert.equal(frame.departed.length, 1);
    assert.equal(frame.departed[0]?.id, "b");
    assert.equal(frame.notify.policy, "single");
  });

  test("drops rows without id rather than refusing the frame", () => {
    const frame = parseFrame(
      JSON.stringify({ protocolVersion:2,
        jobs: [{ name: "no-id" }, { id: "kept" }],
        departed: [],
      }),
    );
    assert.deepEqual(
      frame.jobs.map((job) => job.id),
      ["kept"],
    );
  });

  test("GET /v1/jobs is a bare array", () => {
    const jobs = parseJobsList(JSON.stringify([{ id: "demo-agent-1" }]));
    assert.equal(jobs[0]?.id, "demo-agent-1");
  });

  test("a garbage payload throws rather than painting zero jobs", () => {
    assert.throws(() => parseFrame("not-json"));
  });
});
