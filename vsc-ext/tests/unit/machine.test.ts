import { strict as assert } from "node:assert";
import { parseJob, type Job } from "../../src/model/job";
import { sshHost } from "../../src/model/machine";

function job(value: Record<string, unknown>): Job {
  const parsed = parseJob({ id: "a", ...value });
  if (!parsed) throw new Error("parseJob rejected a test fixture");
  return parsed;
}

suite("which machine", () => {
  test("a job the hub reached over ssh names that Host", () => {
    assert.equal(sshHost(job({ extensions: { sshHost: " Arrhenius " } })), "Arrhenius");
  });

  test("every other job is the hub's own, whatever its alias", () => {
    assert.equal(sshHost(job({ alias: "arrhenius1" })), undefined);
    assert.equal(sshHost(job({ alias: "", extensions: { sshHost: "" } })), undefined);
  });
});
