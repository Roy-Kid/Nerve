import * as vscode from "vscode";
import type { JobFilter } from "../model/filter";
import { compareJobs, jobInFolder, matchesFilter } from "../model/filter";
import type { Status } from "../model/job";
import {
  type Job,
  groupId,
  isGroupJob,
  isMemberJob,
  lastPrompt,
  paintedColor,
} from "../model/job";
import { focusActionTitle } from "../focus/rank";
import { foreignAlias, localAlias } from "../model/machine";
import { statusLabel, statusOf } from "../model/status";
import type { JobStore } from "../store/jobStore";

export type ConnectionKind = "offline" | "empty" | "emptyFolder" | "ready";

export class AliasItem extends vscode.TreeItem {
  constructor(
    readonly alias: string,
    readonly jobs: Job[],
  ) {
    const local = localAlias();
    const label = foreignAlias(alias, local) ? alias : alias || "this machine";
    super(label, vscode.TreeItemCollapsibleState.Expanded);
    this.contextValue = "nerve.alias";
    const worst = jobs.length ? statusOf([...jobs].sort(compareJobs)[0]!) : "inactive";
    this.iconPath = statusIcon(worst);
    this.description = String(jobs.length);
  }
}

export class JobItem extends vscode.TreeItem {
  constructor(
    readonly job: Job,
    known?: Status,
  ) {
    super(job.name || job.id, vscode.TreeItemCollapsibleState.None);
    const status = known ?? statusOf(job);
    this.iconPath = statusIcon(status);
    this.description = rowDescription(job);
    this.tooltip = rowTooltip(job);
    this.contextValue = "nerve.job";
    this.id = job.id;
    if (isGroupJob(job)) {
      this.collapsibleState = vscode.TreeItemCollapsibleState.Collapsed;
    }
  }
}

function rowDescription(job: Job): string {
  const summary = job.current?.summary ?? job.attention.title ?? statusLabel(job);
  const folders = workspaceFolders();
  const here = jobInFolder(job, folders) ? "here · " : "";
  return `${here}${summary}`;
}

function rowTooltip(job: Job): vscode.MarkdownString {
  const bits = [
    job.producer.name ?? job.producer.id,
    job.alias,
    lastPrompt(job),
  ].filter((value): value is string => Boolean(value));
  const markdown = new vscode.MarkdownString();
  markdown.appendMarkdown(`**${job.name || job.id}** · ${statusLabel(job)}\n\n`);
  markdown.appendMarkdown(`${focusActionTitle(job, localAlias())}\n\n`);
  if (bits.length > 0) markdown.appendMarkdown(`${bits.join(" · ")}\n\n`);
  const prompt = lastPrompt(job);
  if (prompt) markdown.appendMarkdown(`Prompt: ${prompt}`);
  markdown.isTrusted = false;
  return markdown;
}

function workspaceFolders(): string[] {
  return (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath);
}

const iconCache = new Map<string, vscode.Uri>();

function statusIcon(status: ReturnType<typeof statusOf>): vscode.Uri {
  const color = paintedColor(status);
  const cached = iconCache.get(color);
  if (cached) return cached;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="5" fill="${color}"/></svg>`;
  const uri = vscode.Uri.parse(`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`);
  iconCache.set(color, uri);
  return uri;
}

export class JobsTreeProvider
  implements vscode.TreeDataProvider<vscode.TreeItem>
{
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  filter: JobFilter = "all";
  connection: ConnectionKind = "offline";

  private projection = "";

  constructor(private readonly store: JobStore) {
    store.onChange(() => {
      const next = treeProjection(this.store.snapshot().jobs, this.filter);
      if (next === this.projection) return;
      this.projection = next;
      this.refresh();
    });
  }

  refresh(): void {
    this.emitter.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: vscode.TreeItem): vscode.TreeItem[] {
    const snapshot = this.store.snapshot();
    const folders = workspaceFolders();
    const ranked: Ranked[] = [];
    const visibleIds = new Set<string>();
    const statusById = new Map<string, Status>();
    for (const job of snapshot.jobs) {
      if (!matchesFilter(job, this.filter, folders)) continue;
      const status = statusOf(job);
      ranked.push({ job, status });
      visibleIds.add(job.id);
      statusById.set(job.id, status);
    }
    ranked.sort((left, right) => compareJobs(left.job, right.job));

    if (!element) {
      this.connection = connectionKind(snapshot.connected, snapshot.jobs.length, ranked.length);
      if (ranked.length === 0) return [];
      const groups = new Map<string, Job[]>();
      for (const { job } of ranked) {
        const parent = groupId(job);
        if (isMemberJob(job) && parent && visibleIds.has(parent)) continue;
        const alias = String((job.extensions.hub as Record<string,unknown>)?.machineGroup ?? "unknown");
        const list = groups.get(alias) ?? [];
        list.push(job);
        groups.set(alias, list);
      }
      const aliases=[...groups.keys()].sort((left,right)=>{
        const a=groups.get(left)?.[0],b=groups.get(right)?.[0];
        return a&&b?compareJobs(a,b):left.localeCompare(right);
      });
      const items = (jobs: Job[]) => jobs.map((job) => new JobItem(job, statusById.get(job.id)));
      if (aliases.length === 1) return items(groups.get(aliases[0]) ?? []);
      return aliases.map((alias) => {const jobs=groups.get(alias) ?? [];return new AliasItem(String((jobs[0]?.extensions.hub as Record<string,unknown>)?.machineLabel ?? alias),jobs);});
    }

    if (element instanceof AliasItem) {
      return element.jobs.map((job) => new JobItem(job, statusById.get(job.id)));
    }
    if (element instanceof JobItem && isGroupJob(element.job)) {
      const members = snapshot.jobs.filter(
        (job) => isMemberJob(job) && groupId(job) === element.job.id,
      );
      members.sort(compareJobs);
      return members.map((job) => new JobItem(job));
    }
    return [];
  }
}

interface Ranked {
  job: Job;
  status: Status;
}

function extensionText(job: Job, key: string): string {
  const value = job.extensions[key];
  return typeof value === "string" ? value : "";
}

function treeProjection(jobs: readonly Job[], filter: JobFilter): string {
  const parts = jobs.map((job) =>
    [
      job.id,
      statusOf(job),
      job.alias,
      job.name,
      job.lifecycle,
      job.current?.type ?? "",
      job.current?.summary ?? "",
      job.attention.level,
      job.attention.title ?? "",
      groupId(job) ?? "",
      extensionText(job, "role"),
    ].join("\u001f"),
  );
  parts.sort();
  return `${filter}\u001e${parts.join("\u001e")}`;
}

function connectionKind(
  connected: boolean,
  total: number,
  visible: number,
): ConnectionKind {
  if (!connected && total === 0) return "offline";
  if (connected && total === 0) return "empty";
  if (visible === 0) return "emptyFolder";
  return "ready";
}
