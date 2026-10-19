/** Adapter for the hub-owned singleton bootstrap. */
export type Launch = "alreadyServing" | "spawned" | "missing" | "failed" | "disabled";
export interface HealthProbe {isServing():boolean|Promise<boolean>}
export interface ProcessSpawner {spawn(program:string,args:readonly string[]):void|Promise<void>}
export class HubLauncher {
 lastError:string|undefined;
 constructor(private readonly spawner:ProcessSpawner,private readonly health:HealthProbe) {}
 async ensure(hub:string|undefined,spawnEnabled:boolean):Promise<Launch> {
  this.lastError=undefined;
  if(!spawnEnabled || !hub) return await this.health.isServing()?"alreadyServing":(!spawnEnabled?"disabled":"missing");
  try {await this.spawner.spawn(hub,["ensure"]);return "spawned";}catch(error){this.lastError=String(error);return "failed";}
 }
}
export function isOffline(launch:Launch):boolean {return launch!=="alreadyServing" && launch!=="spawned";}
