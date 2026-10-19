import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import type { ClientRequest } from "node:http";
import { homedir } from "node:os";
import type { NerveLog } from "../log";
import { parseFrame, parseJobsList, type HubNotification } from "../model/job";
import type { JobStore } from "../store/jobStore";
import { HEALTH_PATH, JOBS_PATH, REFRESH_PATH, STREAM_PATH } from "./endpoint";
import { getStream, getText, postText, sendJson } from "./http";
import { HubLauncher, type Launch } from "./launch";
import { defaultProbe, locateHub } from "./locate";
import { SseParser } from "./sse";

const HEALTH_TIMEOUT_MS = 1_000;
const JOBS_TIMEOUT_MS = 5_000;
const STREAM_IDLE_MS = 60_000;
const MIN_RETRY_MS = 1_000;
const MAX_RETRY_MS = 30_000;

class HttpHealth {
  async isServing(): Promise<boolean> {
    try {
      const health = JSON.parse(await getText(HEALTH_PATH, HEALTH_TIMEOUT_MS));
      return health.service === "nerve" && health.protocolVersion === 2;
    } catch {
      return false;
    }
  }
}

class DetachedSpawner {
 spawn(program:string,args:readonly string[]):Promise<void> {
  return new Promise((resolve,reject)=>{
   const child=spawn(program,[...args],{stdio:["ignore","ignore","pipe"],windowsHide:true});
   let error="";child.stderr?.on("data",data=>error+=String(data));
   child.once("error",reject);child.once("exit",code=>code===0?resolve():reject(new Error(error.trim() || `Hub ensure exited ${code}`)));
  });
 }
}

export interface HubClientOptions {
  extensionPath?: string;
  spawnEnabled: () => boolean;
  log?: NerveLog;
  notificationsEnabled?: () => boolean;
  onNotification?: (event: HubNotification) => Promise<void>;
}

/**
 * One SSE subscription for this extension host. Fail-open: never throws
 * into the editor, never kills a hub.
 */
export class HubClient {
  readonly instanceId = randomUUID();
  private request: ClientRequest | undefined;
  private retryTimer: ReturnType<typeof setTimeout> | undefined;
  private idleTimer: ReturnType<typeof setTimeout> | undefined;
  private retryMs = MIN_RETRY_MS;
  private stopped = false;
  private readonly launcher = new HubLauncher(
    new DetachedSpawner(),
    new HttpHealth(),
  );
  lastLaunch: Launch | undefined;

  constructor(
    private readonly store: JobStore,
    private readonly options: HubClientOptions,
  ) {}

  start(): void {
    this.stopped = false;
    void this.loop();
  }

  stop(): void {
    this.stopped = true;
    if (this.retryTimer) clearTimeout(this.retryTimer);
    if (this.idleTimer) clearTimeout(this.idleTimer);
    this.request?.destroy();
    this.request = undefined;
  }

  reconnect(): void {
    this.stop();
    this.retryMs = MIN_RETRY_MS;
    void this.refreshThenStart();
  }

  private async refreshThenStart(): Promise<void> {
    try {
      const body = await postText(REFRESH_PATH, JOBS_TIMEOUT_MS);
      void body;
      this.store.applyFrame(parseFrame(await getText("/v1/frame", JOBS_TIMEOUT_MS)));
    } catch {
      // The stream reconnect also reads the authoritative full frame.
    }
    this.start();
  }

  private async loop(): Promise<void> {
    if (this.stopped) return;
    const hub = locateHub(defaultProbe(), homedir(), this.options.extensionPath);
    this.lastLaunch = await this.launcher.ensure(hub, this.options.spawnEnabled());
    if(this.launcher.lastError) {this.options.log?.error(this.launcher.lastError);this.store.setOffline(this.launcher.lastError);}
    this.options.log?.info(`hub launch: ${this.lastLaunch}${hub ? ` (${hub})` : ""}`);
    try {
      const body = await getText("/v1/frame", JOBS_TIMEOUT_MS);
      this.store.applyFrame(parseFrame(body));
      await this.configureNotifications();
      this.retryMs = MIN_RETRY_MS;
    } catch (error) {
      this.store.setOffline(String(error));
      this.options.log?.warn(`jobs fetch failed: ${String(error)}`);
      this.store.setOffline();
    }
    if (this.stopped) return;
    this.attachStream();
  }

  private attachStream(): void {
    if (this.stopped) return;
    const parser = new SseParser();
    const bumpIdle = (): void => {
      if (this.idleTimer) clearTimeout(this.idleTimer);
      this.idleTimer = setTimeout(() => {
        this.request?.destroy();
      }, STREAM_IDLE_MS);
    };
    bumpIdle();
    this.request = getStream(
      `${STREAM_PATH}&instanceId=${this.instanceId}&notifications=true`,
      (chunk) => {
        bumpIdle();
        for (const payload of parser.push(chunk)) {
          try {
            const frame = parseFrame(payload);
            if (!this.store.applyFrame(frame)) continue;
            for (const event of frame.notifications ?? []) {
              if (event.target !== this.instanceId) continue;
              void (async () => {
                let delivered = false;
                try { await this.options.onNotification?.(event); delivered = this.options.onNotification !== undefined; } catch {}
                await sendJson("/v1/notifications/result", "POST", {id:event.id, delivered}).catch(()=>{});
              })();
            }
            this.retryMs = MIN_RETRY_MS;
          } catch (error) {
            this.store.setOffline(String(error));
            this.options.log?.error(`frame decode failed: ${String(error)}`);
          }
        }
      },
      () => {
        this.options.log?.info("stream closed");
        this.request = undefined;
        if (this.idleTimer) clearTimeout(this.idleTimer);
        this.store.setOffline();
        this.scheduleRetry();
      },
    );
    this.options.log?.info("stream attached");
  }

  async configureNotifications(): Promise<void> {
    await sendJson(`/v1/clients/${this.instanceId}`, "PUT", {enabled:this.options.notificationsEnabled?.() ?? false});
  }
  private scheduleRetry(): void {
    if (this.stopped) return;
    const wait = this.retryMs;
    this.retryMs = Math.min(this.retryMs * 2, MAX_RETRY_MS);
    this.options.log?.debug(`reconnect in ${wait}ms`);
    this.retryTimer = setTimeout(() => {
      void this.loop();
    }, wait);
  }
}
