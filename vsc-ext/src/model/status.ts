/** Read authoritative projection; presentation labels stay local. */
import type { AttentionLevel, Job, Status } from "./job";
export { STATUS_PRIORITY } from "./job";

function projection(job: Job): Record<string, unknown> { return (job.extensions.hub ?? {}) as Record<string, unknown>; }
export function statusOf(job: Job): Status { return projection(job).status as Status ?? "inactive"; }

/** Product words. Same table as macOS `Status.title`. */
export function statusTitle(status: Status): string {
  switch (status) {
    case "problem":
      return "Problem";
    case "attention":
      return "Needs you";
    case "waiting":
      return "Waiting";
    case "running":
      return "Running";
    case "monitor":
      return "Monitor";
    case "success":
      return "Completed";
    case "inactive":
      return "Ready / idle";
  }
}

/** Reason-specific row word. Same table as macOS `Job.statusLabel`. */
export function statusLabel(job: Job): string { return projection(job).statusLabel as string ?? ""; }

export function isAskElevated(job: Job): boolean { return projection(job).ask === true; }
