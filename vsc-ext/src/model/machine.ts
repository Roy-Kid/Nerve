/**
 * Which machine a job runs on.
 *
 * The hub says. A job it reached through `ssh <Host>` carries that Host as
 * `extensions.sshHost`: a stanza of the user's `~/.ssh/config`, the one file
 * that decides where `ssh` lands, so the same name is also the way there.
 * Every other job is the hub's own machine's — a hook only posts to the hub
 * beside it. No hostname or alias is compared: a Mac renames itself with its
 * network, and a login node (`arrhenius1`) is not spelled the way the Host
 * that reaches it (`Arrhenius`) is.
 *
 * A foreign pid and a foreign workspace path must never be compared against
 * local terminals or local folders. Same question the hub reaper and the
 * tmux surface ask first.
 */

import { extensionString, type Job } from "./job";

/** The ssh Host that reached this job's machine, or `undefined` for the hub's own. */
export function sshHost(job: Job): string | undefined {
  const host = extensionString(job, "sshHost")?.trim();
  return host && host.length > 0 ? host : undefined;
}
