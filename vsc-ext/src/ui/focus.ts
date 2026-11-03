import * as vscode from "vscode";
import { rankFocus, type FocusTarget } from "../focus/rank";
import type { Job } from "../model/job";

export async function performFocus(job: Job): Promise<string> {
  const folders = (vscode.workspace.workspaceFolders ?? []).map(
    (folder) => folder.uri.fsPath,
  );
  const terminals = vscode.window.terminals.map((terminal) => ({
    processId: undefined as number | undefined,
  }));
  await Promise.all(
    vscode.window.terminals.map(async (terminal, index) => {
      try {
        terminals[index] = { processId: await terminal.processId };
      } catch {
        terminals[index] = { processId: undefined };
      }
    }),
  );

  const target = rankFocus(job, {
    workspaceFolders: folders,
    terminals,
    remoteName: vscode.env.remoteName,
    remoteAuthority: remoteAuthority(),
  });
  return applyTarget(target);
}

function remoteAuthority(): string | undefined {
  const uri = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (uri?.scheme === "vscode-remote") return uri.authority;
  return undefined;
}

async function applyTarget(target: FocusTarget): Promise<string> {
  switch (target.kind) {
    case "terminal": {
      const terminal = await terminalWithPid(target.processId);
      terminal?.show();
      return terminal ? "Focused terminal" : "Terminal gone";
    }
    case "here":
      return "Already here";
    case "folder": {
      const uri = vscode.Uri.file(target.path);
      try {
        const stat = await vscode.workspace.fs.stat(uri);
        if (stat.type & vscode.FileType.Directory) {
          await vscode.commands.executeCommand("revealInExplorer", uri);
        } else {
          await vscode.window.showTextDocument(uri, { preview: true });
        }
      } catch {
        await vscode.commands.executeCommand("revealInExplorer", uri);
      }
      return "Opened";
    }
    case "remote": {
      const path = target.cwd && target.cwd.startsWith("/") ? target.cwd : "/";
      const uri = vscode.Uri.parse(
        `vscode-remote://ssh-remote+${target.host}${path}`,
      );
      await vscode.commands.executeCommand("vscode.openFolder", uri, {
        forceNewWindow: true,
      });
      return `Open on ${target.host}`;
    }
    case "copy": {
      await vscode.env.clipboard.writeText(target.text);
      return `Copied location — ${target.reason}`;
    }
  }
}

async function terminalWithPid(
  pid: number,
): Promise<vscode.Terminal | undefined> {
  for (const terminal of vscode.window.terminals) {
    try {
      if ((await terminal.processId) === pid) return terminal;
    } catch {
      continue;
    }
  }
  return undefined;
}

export async function copyLocation(job: Job): Promise<string> {
  const text =
    job.location?.focusHint?.trim() ||
    job.location?.openURL?.trim() ||
    job.id;
  await vscode.env.clipboard.writeText(text);
  return "Copied location";
}

export async function copySummary(job: Job): Promise<string> {
  const detail =
    job.current?.summary ?? job.current?.name ?? job.attention.title ?? "";
  const text = detail ? `${job.name} — ${detail}` : job.name;
  await vscode.env.clipboard.writeText(text);
  return "Copied summary";
}
