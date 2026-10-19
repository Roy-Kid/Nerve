import { decode, isAbsolutePath, pathFromFileUri } from "./path";
/** Wire job subset this surface paints. Lenient: only `id` is required. */

export type Lifecycle =
  | "created"
  | "pending"
  | "active"
  | "suspended"
  | "ended"
  | "unknown";

export type AttentionLevel =
  | "none"
  | "informational"
  | "suggested"
  | "required"
  | "urgent";

export type Health = "ok" | "degraded" | "unresponsive" | "unknown";

export type Outcome = "success" | "failure" | "cancelled" | "partial" | "unknown";

export type Status =
  | "problem"
  | "attention"
  | "waiting"
  | "running"
  | "monitor"
  | "success"
  | "inactive";

export const PAINTED_STATUSES: readonly Status[] = [
  "problem",
  "attention",
  "waiting",
  "running",
  "monitor",
  "success",
  "inactive",
] as const;

export const STATUS_PRIORITY: readonly Status[] = [
  "problem",
  "attention",
  "waiting",
  "running",
  "monitor",
  "success",
  "inactive",
] as const;

export const STATUS_COLORS: Record<Status, string> = {
  problem: "#ff3b30",
  attention: "#ff9f0a",
  waiting: "#8e8e93",
  running: "#0a84ff",
  monitor: "#bf5af2",
  success: "#30d158",
  inactive: "#8e8e93",
};

export function paintedColor(status: Status): string {
  return STATUS_COLORS[status];
}

export interface Current {
  type: string;
  name?: string;
  summary?: string;
  detail?: string;
}

export interface Attention {
  level: AttentionLevel;
  reason?: string;
  title?: string;
  summary?: string;
}

export interface ProducerInfo {
  id: string;
  name?: string;
  kind?: string;
}

export interface JobContext {
  workspace?: string;
  project?: string;
}

export interface LocationInfo {
  openURL?: string;
  focusHint?: string;
  logPath?: string;
}

export interface JobAction {
  id: string;
  title: string;
  kind: string;
}

export interface Job {
  id: string;
  kind: string;
  name: string;
  alias: string;
  lifecycle: Lifecycle;
  current?: Current;
  attention: Attention;
  health: Health;
  outcome?: Outcome;
  producer: ProducerInfo;
  context: JobContext;
  extensions: Record<string, unknown>;
  location?: LocationInfo;
  actions: JobAction[];
  createdAt?: string;
  updatedAt?: string;
}

export type NotifyPolicy = "all" | "single";

export interface NotifyLease {
  policy: NotifyPolicy;
  owner?: string;
  surfaces: string[];
  watchers: number;
}

/** Older hub omitted `notify`; every surface may fire. */
export const EMPTY_NOTIFY: NotifyLease = {
  policy: "single",
  surfaces: [],
  watchers: 0,
};

export interface HubNotification { id: string; target: string; jobId: string; title: string; body: string; level: string; sound: boolean }
export interface Frame {
  hubInstanceId?:string;
  revision?:number;
  notifications?: HubNotification[];
  summary?: Record<string, unknown>;
  jobs: Job[];
  departed: Job[];
  notify: NotifyLease;
}

const LIFECYCLES = new Set<Lifecycle>([
  "created",
  "pending",
  "active",
  "suspended",
  "ended",
  "unknown",
]);
const ATTENTION_LEVELS = new Set<AttentionLevel>([
  "none",
  "informational",
  "suggested",
  "required",
  "urgent",
]);
const HEALTHS = new Set<Health>(["ok", "degraded", "unresponsive", "unknown"]);
const OUTCOMES = new Set<Outcome>([
  "success",
  "failure",
  "cancelled",
  "partial",
  "unknown",
]);

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return undefined;
}

function asString(value: unknown): string | undefined {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length === 0 ? undefined : trimmed;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }
  return undefined;
}

function enumValue<T extends string>(
  value: unknown,
  allowed: Set<T>,
  fallback: T,
): T {
  const raw = asString(value)?.toLowerCase();
  if (raw && allowed.has(raw as T)) return raw as T;
  return fallback;
}

function parseCurrent(value: unknown): Current | undefined {
  const record = asRecord(value);
  if (!record) return undefined;
  const type = asString(record.type) ?? "";
  return {
    type,
    name: asString(record.name),
    summary: asString(record.summary),
    detail: asString(record.detail),
  };
}

function parseAttention(value: unknown): Attention {
  const record = asRecord(value);
  if (!record) return { level: "none" };
  return {
    level: enumValue(record.level, ATTENTION_LEVELS, "none"),
    reason: asString(record.reason),
    title: asString(record.title),
    summary: asString(record.summary),
  };
}

function parseProducer(value: unknown): ProducerInfo {
  const record = asRecord(value);
  if (!record) return { id: "" };
  return {
    id: asString(record.id) ?? "",
    name: asString(record.name),
    kind: asString(record.kind),
  };
}

function parseContext(value: unknown): JobContext {
  const record = asRecord(value);
  if (!record) return {};
  return {
    workspace: asString(record.workspace),
    project: asString(record.project),
  };
}

function parseLocation(value: unknown): LocationInfo | undefined {
  const record = asRecord(value);
  if (!record) return undefined;
  const location: LocationInfo = {
    openURL: asString(record.openURL),
    focusHint: asString(record.focusHint),
    logPath: asString(record.logPath),
  };
  if (!location.openURL && !location.focusHint && !location.logPath) {
    return undefined;
  }
  return location;
}

function parseActions(value: unknown): JobAction[] {
  if (!Array.isArray(value)) return [];
  const actions: JobAction[] = [];
  for (const item of value) {
    const record = asRecord(item);
    if (!record) continue;
    actions.push({
      id: asString(record.id) ?? "",
      title: asString(record.title) ?? "",
      kind: asString(record.kind) ?? "",
    });
  }
  return actions;
}

function parseExtensions(value: unknown): Record<string, unknown> {
  return asRecord(value) ?? {};
}

export function parseJob(value: unknown): Job | undefined {
  const record = asRecord(value);
  if (!record) return undefined;
  const id = asString(record.id);
  if (!id) return undefined;
  const outcomeRaw = record.outcome;
  return {
    id,
    kind: asString(record.kind) ?? "session",
    name: asString(record.name) ?? "",
    alias: asString(record.alias) ?? "",
    lifecycle: enumValue(record.lifecycle, LIFECYCLES, "active"),
    current: parseCurrent(record.current),
    attention: parseAttention(record.attention),
    health: enumValue(record.health, HEALTHS, "ok"),
    outcome:
      outcomeRaw === undefined || outcomeRaw === null
        ? undefined
        : enumValue(outcomeRaw, OUTCOMES, "unknown"),
    producer: parseProducer(record.producer),
    context: parseContext(record.context),
    extensions: { ...parseExtensions(record.extensions), hub: record.display },
    location: parseLocation(record.location),
    actions: parseActions(record.actions),
    createdAt: asString(record.createdAt),
    updatedAt: asString(record.updatedAt),
  };
}

export function parseFrame(raw: string): Frame {
  const value = JSON.parse(raw) as unknown;
  const record = asRecord(value);
  if (!record) {
    throw new Error("frame is not an object");
  }
  if (record.protocolVersion !== 2) throw new Error("Upgrade Nerve hub: protocol version 2 required");
  const jobs = Array.isArray(record.jobs)
    ? record.jobs.map(parseJob).filter((job): job is Job => job !== undefined)
    : [];
  const departed = Array.isArray(record.departed)
    ? record.departed
        .map(parseJob)
        .filter((job): job is Job => job !== undefined)
    : [];
  return {hubInstanceId:asString(record.hubInstanceId),revision:typeof record.revision==="number"?record.revision:0, jobs, departed, notify: parseNotify(record.notify), notifications: Array.isArray(record.notifications) ? record.notifications as HubNotification[] : [], summary: asRecord(record.summary) ?? {} };
}

function parseNotify(value: unknown): NotifyLease {
  const record = asRecord(value);
  if (!record) return EMPTY_NOTIFY;
  const raw = typeof record.policy === "string" ? record.policy.trim().toLowerCase() : "";
  // Missing or unknown policy on a present object is single, the hub default.
  // A missing object also uses the hub default.
  const policy: NotifyPolicy = raw === "all" ? "all" : "single";
  const owner = asString(record.owner);
  const surfaces = Array.isArray(record.surfaces)
    ? record.surfaces.filter((item): item is string => typeof item === "string")
    : [];
  const watchers =
    typeof record.watchers === "number" && Number.isFinite(record.watchers)
      ? record.watchers
      : 0;
  return { policy, owner, surfaces, watchers };
}

export function parseJobsList(raw: string): Job[] {
  const value = JSON.parse(raw) as unknown;
  if (!Array.isArray(value)) {
    throw new Error("jobs list is not an array");
  }
  return value.map(parseJob).filter((job): job is Job => job !== undefined);
}

export function extensionString(
  job: Job,
  key: string,
): string | undefined {
  const value = job.extensions[key];
  return asString(value);
}

export function jobPid(job: Job): number | undefined {
  const value = job.extensions.pid;
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.trunc(value);
  }
  if (typeof value === "string") {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed) && parsed > 0) return parsed;
  }
  return undefined;
}

export function lastPrompt(job: Job): string | undefined {
  return extensionString(job, "lastPrompt");
}

export function extensionRole(job: Job): string | undefined { return (job.extensions.hub as {role?:string}|undefined)?.role; }
export function isGroupJob(job: Job): boolean { return extensionRole(job) === "group"; }
export function isMemberJob(job: Job): boolean { return extensionRole(job) === "member"; }
export function groupId(job: Job): string | undefined { return (job.extensions.hub as {groupId?:string}|undefined)?.groupId; }

export function jobWorkspace(job: Job): string | undefined {
  return (
    job.context.workspace ??
    pathFromOpenUrl(job.location?.openURL)
  );
}

export function pathFromOpenUrl(url: string | undefined): string | undefined {
  if (!url) return undefined;
  const trimmed = url.trim();
  if (trimmed.startsWith("file://")) return pathFromFileUri(trimmed);

  const ide = /^(?:vscode|cursor|vscode-insiders):\/\/file(\/.*)$/i.exec(
    trimmed,
  );
  if (ide) {
    const decoded = decode(ide[1]);
    // `vscode://file/C:/work` — the leading slash is the URL's, not the path's.
    const afterSlash = decoded.slice(1);
    if (isAbsolutePath(afterSlash)) return afterSlash;
    if (isAbsolutePath(decoded)) return decoded;
    return undefined;
  }
  if (isAbsolutePath(trimmed)) return trimmed;
  return undefined;
}
