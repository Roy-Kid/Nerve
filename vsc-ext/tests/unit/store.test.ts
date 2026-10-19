import { strict as assert } from "node:assert";
import { parseFrame } from "../../src/model/job";
import { JobStore } from "../../src/store/jobStore";

suite("job store", () => {
  test("a frame replaces the set", () => {
    const store = new JobStore();
    store.applyFrame(
      parseFrame(JSON.stringify({ protocolVersion:2, jobs: [{ id: "a" }, { id: "b" }] })),
    );
    assert.equal(store.snapshot().jobs.length, 2);
    store.applyFrame(parseFrame(JSON.stringify({ protocolVersion:2, jobs: [{ id: "c" }] })));
    assert.deepEqual(
      store.snapshot().jobs.map((job) => job.id),
      ["c"],
    );
    assert.equal(store.snapshot().connected, true);
  });

  test("offline keeps the last jobs", () => {
    const store = new JobStore();
    store.applyFrame(
      parseFrame(JSON.stringify({ protocolVersion:2, jobs: [{ id: "a" }] })),
    );
    store.setOffline();
    assert.equal(store.snapshot().connected, false);
    assert.equal(store.snapshot().jobs[0]?.id, "a");
  });
  test("older revisions cannot replace a snapshot, and a restarted hub can", () => {
    const store = new JobStore();
    const frame = (hubInstanceId:string, revision:number, id:string) => parseFrame(JSON.stringify({protocolVersion:2,hubInstanceId,revision,jobs:[{id}]}));
    assert.equal(store.applyFrame(frame("first",10,"current")),true);
    assert.equal(store.applyFrame(frame("first",9,"stale")),false);
    assert.equal(store.snapshot().jobs[0]?.id,"current");
    assert.equal(store.applyFrame(frame("restarted",1,"fresh")),true);
    assert.equal(store.snapshot().jobs[0]?.id,"fresh");
  });
  test("protocol errors are visible even before the first successful frame", () => {
    const store = new JobStore();let changes=0;store.onChange(()=>changes++);
    store.setOffline("Upgrade Nerve hub");assert.equal(changes,1);assert.match(store.snapshot().error!,/Upgrade/);
  });
});
