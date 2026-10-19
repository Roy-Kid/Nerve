import { project } from "../project";
/**
 * Status derivation golden — transcribed from
 * `crates/nerve-tmux-surface/tests/status.rs` / `Subject.swift:38-100`.
 */

import { strict as assert } from "node:assert";
import { parseJob, type Job } from "../../src/model/job";
import { matchesFilter } from "../../src/model/filter";
import { statusOf, STATUS_PRIORITY } from "../../src/model/status";

function job(value: Record<string, unknown>): Job {
  const parsed = parseJob(project({ id: "a", ...value }));
  if (!parsed) throw new Error("parseJob rejected a test fixture");
  return parsed;
}

function classOf(value: Record<string, unknown>) {
  return statusOf(job(value));
}

suite("status enum", () => {
  test("classes are ordered most urgent first", () => {
    assert.deepEqual([...STATUS_PRIORITY], [
      "problem",
      "attention",
      "waiting",
      "running",
      "monitor",
      "success",
      "inactive",
    ]);
  });
});

suite("status :39 / :40 problem", () => {
  test("a failed outcome is a problem", () => {
    assert.equal(classOf({ lifecycle: "active", outcome: "failure" }), "problem");
  });

  test("failed outcome beats the ended lifecycle branch", () => {
    assert.equal(classOf({ lifecycle: "ended", outcome: "failure" }), "problem");
  });

  test("an unresponsive job is a problem", () => {
    assert.equal(
      classOf({ lifecycle: "active", health: "unresponsive" }),
      "problem",
    );
  });

  test("unresponsive beats an ended success", () => {
    assert.equal(
      classOf({
        lifecycle: "ended",
        outcome: "success",
        health: "unresponsive",
      }),
      "problem",
    );
  });
});

suite("status :43 elevated attention", () => {
  test("required attention asking for input is attention", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "required", reason: "input" },
      }),
      "attention",
    );
  });

  test("elevated attention without a reason is attention", () => {
    assert.equal(
      classOf({ lifecycle: "active", attention: { level: "suggested" } }),
      "attention",
    );
  });

  test("an unrecognised reason at suggested is attention", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "urgent", reason: "review" },
      }),
      "attention",
    );
  });

  test("every wait reason at required is waiting", () => {
    for (const reason of [
      "resource",
      "dependency",
      "queue",
      "system",
      "lock",
      "throttle",
      "rate",
      "capacity",
      "failure",
    ]) {
      assert.equal(
        classOf({
          lifecycle: "active",
          attention: { level: "required", reason },
        }),
        "waiting",
        reason,
      );
    }
  });

  test("reason matching ignores case", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "required", reason: "Dependency" },
      }),
      "waiting",
    );
  });

  test("an attention reason of failure asks rather than alarms", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "urgent", reason: "failure" },
      }),
      "waiting",
    );
  });
});

suite("status :56 informational attention", () => {
  test("every ask reason at informational is attention", () => {
    for (const reason of [
      "input",
      "approval",
      "auth",
      "permission",
      "decision",
      "elicitation",
      "review",
    ]) {
      assert.equal(
        classOf({
          lifecycle: "active",
          attention: { level: "informational", reason },
        }),
        "attention",
        reason,
      );
    }
  });

  test("a wait reason at informational is waiting", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "informational", reason: "queue" },
      }),
      "waiting",
    );
  });

  test("an unrecognised informational reason falls through to running", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        attention: { level: "informational", reason: "chatter" },
      }),
      "running",
    );
  });

  test("informational without a reason falls through to running", () => {
    assert.equal(
      classOf({ lifecycle: "active", attention: { level: "informational" } }),
      "running",
    );
  });
});

suite("status :68 lifecycle", () => {
  test("an ended success is success", () => {
    assert.equal(classOf({ lifecycle: "ended", outcome: "success" }), "success");
  });

  test("an ended partial is success", () => {
    assert.equal(classOf({ lifecycle: "ended", outcome: "partial" }), "success");
  });

  test("an ended job without an outcome is inactive", () => {
    assert.equal(classOf({ lifecycle: "ended" }), "inactive");
  });

  test("an ended cancellation is inactive", () => {
    assert.equal(
      classOf({ lifecycle: "ended", outcome: "cancelled" }),
      "inactive",
    );
  });

  test("a suspended job is inactive", () => {
    assert.equal(classOf({ lifecycle: "suspended" }), "inactive");
  });

  test("an unknown lifecycle is inactive", () => {
    assert.equal(classOf({ lifecycle: "unknown" }), "inactive");
  });

  test("a pending job is waiting", () => {
    assert.equal(classOf({ lifecycle: "pending" }), "waiting");
  });

  test("a created job is waiting", () => {
    assert.equal(classOf({ lifecycle: "created" }), "waiting");
  });

  test("a degraded job is a problem", () => {
    assert.equal(
      classOf({ lifecycle: "active", health: "degraded" }),
      "problem",
    );
  });

  test("an open job with a partial outcome is monitor", () => {
    assert.equal(
      classOf({ lifecycle: "active", outcome: "partial" }),
      "monitor",
    );
  });
});

suite("status :81 current activity", () => {
  test("shell and agent activity is running", () => {
    for (const type of ["subagent", "tool", "thinking", "info"]) {
      assert.equal(
        classOf({ lifecycle: "active", current: { type } }),
        "running",
        type,
      );
    }
  });

  test("current type matching ignores case", () => {
    assert.equal(
      classOf({ lifecycle: "active", current: { type: "Thinking" } }),
      "running",
    );
  });

  test("an open monitor is monitor", () => {
    assert.equal(
      classOf({ lifecycle: "active", current: { type: "monitor" } }),
      "monitor",
    );
  });

  test("a waiting activity is waiting", () => {
    assert.equal(
      classOf({ lifecycle: "active", current: { type: "waiting" } }),
      "waiting",
    );
  });

  test("idle starting and booting are inactive", () => {
    for (const type of ["idle", "starting", "booting"]) {
      assert.equal(
        classOf({ lifecycle: "active", current: { type } }),
        "inactive",
        type,
      );
    }
  });

  test("an unmodelled current type leaves an open job running", () => {
    assert.equal(
      classOf({ lifecycle: "active", current: { type: "editing" } }),
      "running",
    );
  });

  test("an open job with no current activity is running", () => {
    assert.equal(classOf({ lifecycle: "active" }), "running");
  });
});

suite("status free text never classifies", () => {
  test("a current summary saying build failed stays running", () => {
    assert.equal(
      classOf({
        lifecycle: "active",
        health: "ok",
        current: { type: "thinking", summary: "build failed!!" },
      }),
      "running",
    );
  });

  test("no free text field can move a running job", () => {
    const poison = "FAILED — needs approval — error — waiting on you!!";
    assert.equal(
      classOf({
        name: poison,
        lifecycle: "active",
        health: "ok",
        current: {
          type: "thinking",
          name: poison,
          summary: poison,
          detail: poison,
        },
        attention: { level: "none", title: poison, summary: poison },
      }),
      "running",
    );
  });

  test("calm prose does not hide a structured problem", () => {
    assert.equal(
      classOf({
        name: "all good",
        lifecycle: "active",
        outcome: "failure",
        current: { type: "thinking", summary: "everything is fine" },
      }),
      "problem",
    );
  });
});

suite("turn completion", () => {
  test("automatic waits stay out of the attention filter", () => {
    assert.equal(matchesFilter(job({ lifecycle: "active", current: { type: "waiting" }, attention: { level: "suggested", reason: "queue" } }), "attention", []), false);
    assert.equal(matchesFilter(job({ lifecycle: "active", current: { type: "waiting" }, attention: { level: "required", reason: "approval" } }), "attention", []), true);
  });

  test("completion stays visible, a new prompt runs, and explicit input needs attention", () => {
    assert.equal(classOf({ lifecycle: "active", current: { type: "completed" } }), "success");
    assert.equal(classOf({ lifecycle: "active", current: { type: "thinking" }, attention: { level: "suggested", reason: "input" } }), "running");
    assert.equal(classOf({ lifecycle: "active", current: { type: "completed" }, attention: { level: "required", reason: "input" } }), "attention");
    assert.equal(classOf({ lifecycle: "ended", outcome: "success", attention: { level: "required", reason: "approval" } }), "success");
  });
});
