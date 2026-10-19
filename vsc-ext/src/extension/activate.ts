import * as vscode from "vscode";
import type { JobFilter } from "../model/filter";
import type { Job } from "../model/job";
import { isAskElevated, statusOf } from "../model/status";
import { HubClient } from "../hub/client";
import { createNerveLog } from "../log";
import { AskToast } from "../notify/askToast";
import { JobStore } from "../store/jobStore";
import { copyLocation, copySummary, performFocus } from "../ui/focus";
import { JobItem, JobsTreeProvider } from "../ui/jobsTree";
import { NerveStatusBar } from "../ui/statusBar";

const FILTERS: JobFilter[] = ["all", "attention", "running", "folder"];

export function activate(context: vscode.ExtensionContext): void {
  const store = new JobStore();
  const tree = new JobsTreeProvider(store);
  tree.filter = readFilter();
  const treeView = vscode.window.createTreeView("nerve.jobs", {
    treeDataProvider: tree,
    showCollapseAll: false,
  });
  const statusBar = new NerveStatusBar(store, () =>
    vscode.workspace.getConfiguration("nerve").get("statusBar.enabled", true),
  );
  const askToast = new AskToast();
  const log = createNerveLog();
  const client = new HubClient(store, {
    extensionPath: context.extensionPath,
    spawnEnabled: () =>
      vscode.workspace.getConfiguration("nerve").get("hub.spawn", true),
    log,
    notificationsEnabled: () => vscode.workspace.getConfiguration("nerve").get("notifications.ask",true),
    onNotification: event => askToast.present(event, store.snapshot().jobs.find(job=>job.id === event.jobId)),
  });

  const applyChrome = (): void => {
    const snapshot = store.snapshot();


    const counts = (snapshot.summary.counts ?? {}) as Record<string,number>;
    const badgeCount = (counts.problem ?? 0) + Number(snapshot.summary.ask ?? 0);
    const badgeOn = vscode.workspace
      .getConfiguration("nerve")
      .get("badge.enabled", true);
    treeView.badge =
      badgeOn && snapshot.connected && badgeCount > 0
        ? {
            value: badgeCount,
            tooltip:
              badgeCount === 1
                ? "1 ready for you"
                : `${badgeCount} ready for you`,
          }
        : undefined;
    void vscode.commands.executeCommand(
      "setContext",
      "nerve.connection",
      tree.connection,
    );
  };

  store.onChange(applyChrome);
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(event => { if (event.affectsConfiguration("nerve.notifications")) void client.configureNotifications(); }));

  const jobFromArg = (arg: unknown): Job | undefined => {
    if (arg instanceof JobItem) return arg.job;
    if (treeView.selection[0] instanceof JobItem) return treeView.selection[0].job;
    return undefined;
  };

  const setFilter = async (filter: JobFilter): Promise<void> => {
    tree.filter = filter;
    await vscode.workspace
      .getConfiguration("nerve")
      .update("filter", filter, vscode.ConfigurationTarget.Global);
    tree.refresh();
    applyChrome();
  };

  context.subscriptions.push(
    log,
    treeView,
    statusBar,
    vscode.commands.registerCommand("nerve.showJobs", async () => {
      await vscode.commands.executeCommand("nerve.jobs.focus");
    }),
    vscode.commands.registerCommand("nerve.focusJob", async (arg: unknown) => {
      const job = jobFromArg(arg);
      if (!job) return;
      const message = await performFocus(job);
      vscode.window.setStatusBarMessage(`Nerve: ${message}`, 2500);
    }),
    vscode.commands.registerCommand("nerve.focusJobQuick", async () => {
      const items = store.snapshot().jobs.map((job) => ({
        label: job.name || job.id,
        description: statusOf(job),
        detail: job.current?.summary ?? job.alias,
        job,
      }));
      const picked = await vscode.window.showQuickPick(items, {
        title: "Focus job",
        matchOnDescription: true,
        matchOnDetail: true,
      });
      if (!picked) return;
      const message = await performFocus(picked.job);
      vscode.window.setStatusBarMessage(`Nerve: ${message}`, 2500);
    }),
    vscode.commands.registerCommand("nerve.copyLocation", async (arg: unknown) => {
      const job = jobFromArg(arg);
      if (!job) return;
      vscode.window.setStatusBarMessage(`Nerve: ${await copyLocation(job)}`, 2500);
    }),
    vscode.commands.registerCommand("nerve.copySummary", async (arg: unknown) => {
      const job = jobFromArg(arg);
      if (!job) return;
      vscode.window.setStatusBarMessage(`Nerve: ${await copySummary(job)}`, 2500);
    }),
    vscode.commands.registerCommand("nerve.reconnect", () => {
      client.reconnect();
    }),
    vscode.commands.registerCommand("nerve.filterAll", () => setFilter("all")),
    vscode.commands.registerCommand("nerve.filterAttention", () =>
      setFilter("attention"),
    ),
    vscode.commands.registerCommand("nerve.filterRunning", () =>
      setFilter("running"),
    ),
    vscode.commands.registerCommand("nerve.filterFolder", () =>
      setFilter("folder"),
    ),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("nerve")) {
        const next = readFilter();
        if (next !== tree.filter) {
          tree.filter = next;
          tree.refresh();
        }
        statusBar.render();
        applyChrome();
      }
    }),
    { dispose: () => client.stop() },
  );

  applyChrome();
  client.start();
}

export function deactivate(): void {
  // HubClient.stop runs through the subscription disposer. Never kill the hub.
}

function readFilter(): JobFilter {
  const value = vscode.workspace.getConfiguration("nerve").get<string>("filter", "all");
  return FILTERS.includes(value as JobFilter) ? (value as JobFilter) : "all";
}
