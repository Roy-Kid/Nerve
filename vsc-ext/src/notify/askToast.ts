import * as vscode from "vscode";
import type { HubNotification, Job } from "../model/job";
import { performFocus } from "../ui/focus";
/** Execute an already-decided hub notification. No transition state lives here. */
export class AskToast {
  async present(event: HubNotification, job: Job | undefined): Promise<void> {
    const message = event.body ? `${event.title} — ${event.body}` : event.title;
    const selected = event.level === "required" || event.level === "urgent"
      ? await vscode.window.showWarningMessage(message, "Open")
      : await vscode.window.showInformationMessage(message, "Open");
    if (selected === "Open" && job) {
      vscode.window.setStatusBarMessage(`Nerve: ${await performFocus(job)}`,2500);
    }
  }
}
