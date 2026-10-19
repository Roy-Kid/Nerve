import { strict as assert } from "node:assert";
import { parseJob } from "../../src/model/job";
import { isAskElevated,statusOf } from "../../src/model/status";
import { project } from "../project";
suite("hub authority",()=>{
 test("clients consume hub Ask projection",()=>{const job=parseJob(project({id:"a",attention:{level:"required",reason:"approval"}}))!;assert.equal(isAskElevated(job),true);});
 test("clients never override the hub projection from raw facets",()=>{const job=parseJob(project({id:"a",current:{type:"thinking"}}))!;job.outcome="failure";job.health="unresponsive";assert.equal(statusOf(job),"running");});
});
