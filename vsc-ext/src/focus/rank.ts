import {
  type Job,
  jobPid,
  jobWorkspace,
  pathFromOpenUrl,
} from "../model/job";
import { sshHost } from "../model/machine";
import { isAbsolutePath, isInside } from "../model/path";

export interface TerminalHint {
  processId?: number;
}

export interface FocusContext {
  workspaceFolders: readonly string[];
  terminals: readonly TerminalHint[];
  /** `vscode.env.remoteName`, e.g. `ssh-remote` / `wsl`. */
  remoteName?: string;
  /** Authority for a vscode-remote window (`ssh-remote+Host`). */
  remoteAuthority?: string;
}

export type FocusTarget =
  | { kind: "terminal"; processId: number }
  | { kind: "here" }
  | { kind: "folder"; path: string }
  | { kind: "remote"; host: string; cwd?: string }
  | { kind: "copy"; text: string; reason: string };

function inFolders(path: string, folders: readonly string[]): boolean {
  return folders.some((folder) => isInside(path, folder));
}

function breadcrumb(job: Job, fallback: string): FocusTarget {
  const hint = job.location?.focusHint?.trim();
  const url = job.location?.openURL?.trim();
  const text = hint || url || "";
  return {
    kind: "copy",
    text: text.length > 0 ? text : fallback,
    reason: fallback,
  };
}

/** This window is a remote one opened on the same ssh Host. */
function remoteWindowMatches(host: string, ctx: FocusContext): boolean {
  if (!ctx.remoteName || !ctx.remoteAuthority) return false;
  const authority = ctx.remoteAuthority;
  const window = authority.includes("+")
    ? authority.slice(authority.indexOf("+") + 1)
    : authority;
  return window.length > 0 && window.toLowerCase() === host.toLowerCase();
}

function rankLocal(job: Job, ctx: FocusContext): FocusTarget {
  const pid = jobPid(job);
  if (pid !== undefined) {
    const terminal = ctx.terminals.find((item) => item.processId === pid);
    if (terminal) return { kind: "terminal", processId: pid };
  }

  const workspace = jobWorkspace(job);
  if (workspace && inFolders(workspace, ctx.workspaceFolders)) {
    return { kind: "here" };
  }

  const path = pathFromOpenUrl(job.location?.openURL) ?? workspace;
  if (path && isAbsolutePath(path)) {
    return { kind: "folder", path };
  }

  return breadcrumb(job, "return to the agent UI");
}

export function rankFocus(job: Job, ctx: FocusContext): FocusTarget {
  const host = sshHost(job);
  if (!host || remoteWindowMatches(host, ctx)) return rankLocal(job, ctx);
  return { kind: "remote", host, cwd: jobWorkspace(job) };
}

export function focusActionTitle(job: Job): string {
  const host = sshHost(job);
  if (host) return `Open on ${host}`;
  if (job.location?.openURL) return "Open";
  return "Focus";
}
