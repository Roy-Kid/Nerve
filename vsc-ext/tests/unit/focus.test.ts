import { strict as assert } from "node:assert";
import { rankFocus, focusActionTitle } from "../../src/focus/rank";
import { parseJob, type Job } from "../../src/model/job";

function job(value: Record<string, unknown>): Job {
  const parsed = parseJob({ id: "a", alias: "macbook", ...value });
  if (!parsed) throw new Error("parseJob rejected a test fixture");
  return parsed;
}

const local = {
  workspaceFolders: ["/Users/me/work/nerve"],
  terminals: [] as { processId?: number }[],
};

/** A job the hub reached through `ssh Arrhenius`. */
const onArrhenius = { alias: "arrhenius1", extensions: { sshHost: "Arrhenius" } };

suite("focus ranking", () => {
  test("pid matching a terminal wins", () => {
    const target = rankFocus(
      job({ extensions: { pid: 99 }, context: { workspace: "/Users/me/work/nerve" } }),
      { ...local, terminals: [{ processId: 99 }] },
    );
    assert.deepEqual(target, { kind: "terminal", processId: 99 });
  });

  test("a workspace hit is here", () => {
    const target = rankFocus(
      job({ context: { workspace: "/Users/me/work/nerve" } }),
      local,
    );
    assert.deepEqual(target, { kind: "here" });
  });

  test("a foreign pid is never matched locally", () => {
    const target = rankFocus(
      job({
        alias: "arrhenius1",
        extensions: { pid: 99, sshHost: "Arrhenius" },
        context: { workspace: "/nobackup/proj" },
      }),
      { ...local, terminals: [{ processId: 99 }] },
    );
    assert.deepEqual(target, { kind: "remote", host: "Arrhenius", cwd: "/nobackup/proj" });
  });

  test("a remote window on that machine is treated as local", () => {
    const target = rankFocus(
      job({ ...onArrhenius, context: { workspace: "/nobackup/proj" } }),
      {
        ...local,
        workspaceFolders: ["/nobackup/proj"],
        remoteName: "ssh-remote",
        remoteAuthority: "ssh-remote+Arrhenius",
      },
    );
    assert.deepEqual(target, { kind: "here" });
  });
});
