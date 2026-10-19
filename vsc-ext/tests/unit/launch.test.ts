import { strict as assert } from "node:assert";
import {HubLauncher} from "../../src/hub/launch";
suite("hub bootstrap delegation",()=>{
 test("always delegates singleton checks and readiness to ensure",async()=>{
  const calls:string[][]=[];const launcher=new HubLauncher({spawn:(_p,args)=>{calls.push([...args]);}},{isServing:()=>false});
  assert.equal(await launcher.ensure("hub",true),"spawned");assert.equal(await launcher.ensure("hub",true),"spawned");assert.deepEqual(calls,[["ensure"],["ensure"]]);
 });
 test("retains an incompatible protocol error",async()=>{
  const launcher=new HubLauncher({spawn:()=>{throw new Error("Upgrade Nerve hub");}},{isServing:()=>false});
  assert.equal(await launcher.ensure("hub",true),"failed");assert.match(launcher.lastError!,/Upgrade/);
 });
});
