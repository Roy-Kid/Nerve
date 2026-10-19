import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
export function project(value: Record<string, unknown>): Record<string, unknown> {
 const result=spawnSync(resolve("../target/debug/examples/project"),[],{input:JSON.stringify(value),encoding:"utf8"});
 if(result.status!==0) throw new Error(result.stderr);
 return JSON.parse(result.stdout) as Record<string,unknown>;
}
