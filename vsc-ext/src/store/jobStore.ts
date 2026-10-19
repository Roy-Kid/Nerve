import { EMPTY_NOTIFY, type Frame, type Job, type NotifyLease } from "../model/job";

export interface StoreSnapshot {
  jobs: readonly Job[];
  connected: boolean;
  notify: NotifyLease;
  summary: Record<string, unknown>;
  error?:string;
}

type Listener = () => void;

/** Frame-fed read-only cache. Writes leave only by replacing the frame. */
export class JobStore {
  private jobs: Job[] = [];
  private connected = false;
  private error:string|undefined;
  private hubInstance:string|undefined;
  private revision=0;
  private notify: NotifyLease = EMPTY_NOTIFY;
  private summary: Record<string, unknown> = {};
  private readonly listeners = new Set<Listener>();

  snapshot(): StoreSnapshot {
    return { jobs: this.jobs, connected: this.connected, notify: this.notify, summary: this.summary,error:this.error };
  }

  applyFrame(frame: Frame): boolean {
    if(frame.hubInstanceId===this.hubInstance && (frame.revision ?? 0)<this.revision)return false;
    this.hubInstance=frame.hubInstanceId;this.revision=frame.revision ?? 0;
    this.error=undefined;
    this.jobs = frame.jobs;
    this.notify = frame.notify;
    this.summary = frame.summary ?? {};
    this.connected = true;
    this.emit();
    return true;
  }

  setOffline(error?:string): void {
    if(error)this.error=error;
    if (!error && !this.connected && this.jobs.length === 0) return;
    this.connected = false;
    this.emit();
  }

  onChange(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}
