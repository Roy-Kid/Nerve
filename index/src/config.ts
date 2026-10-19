import hubFixture from '../../fixtures/website-frame.json';
import type { Locale } from './i18n/messages';

/** Product links and copy — edit when URLs change. */
export const site = {
  name: 'Nerve',
  /** Primary hero line (split for display). */
  headline: ['Every job.', 'One glance.'],
  subhead: 'Running, needs you, or broken—right in the menu bar.',
  github: 'https://github.com/Roy-Kid/nerve',
  /** Replace with the live App Store product URL when published. */
  appStore: 'https://apps.apple.com/app/nerve',
  appStoreReady: false,
  /** In-site documentation hub (subpages under /docs). */
  docs: '/docs',
  pluginDocs: '/docs/plugin',
  macos: '/#macos',
  tmux: '/#tmux',
  vscode: '/#vscode',
  tmuxGuideZh: '/#tmux',
  tmuxGuideEn: '/#tmux',
  requirements: {
    macos: 'macOS 14+',
    xcode: 'Xcode 15+',
  },
} as const;

export function getTmuxGuidePath(_locale?: Locale) {
  return site.tmux;
}

/** Examples consume a hub-generated protocol fixture; colours are presentation. */
export type StatusTone = 'problem' | 'attention' | 'running' | 'monitor' | 'success' | 'inactive';
export type JobStatus = StatusTone;
export const jobs = hubFixture.jobs.map(job => ({
  id:job.id, label:job.name, alias:job.alias, status:job.display.status as StatusTone,
}));

/** Idle is a rest, not a hue — half the width of every painted status. */
export function ribbonWeight(status: StatusTone): number {
  return status === 'inactive' ? 0.5 : 1;
}

/** Six painted hues. Rainbow red / orange / blue / violet / green, plus gray idle. */
export const statusMeta: Record<
  StatusTone,
  { label: string; color: string; hint: string }
> = {
  problem: { label: 'Problem', color: 'var(--color-status-problem)', hint: 'Failed / cannot continue' },
  attention: { label: 'Needs you', color: 'var(--color-status-attention)', hint: 'Needs you, or stuck' },
  running: { label: 'Running', color: 'var(--color-status-running)', hint: 'In flight' },
  monitor: { label: 'Monitor', color: 'var(--color-status-monitor)', hint: 'Watching a background stream' },
  success: { label: 'Completed', color: 'var(--color-status-success)', hint: 'Finished' },
  inactive: { label: 'Ready / idle', color: 'var(--color-status-inactive)', hint: 'Ready / quiet' },
};

export const truths = [
  {
    title: 'One live ribbon.',
    body: 'See every job at a glance.',
  },
  {
    title: 'Only when it matters.',
    body: 'Running, needs you, and failed stay distinct.',
  },
  {
    title: 'Open and local.',
    body: 'The protocol and source are yours to inspect.',
  },
] as const;

export const sources = [
  { name: 'Claude Code', how: 'Marketplace plugin · fail-open hooks' },
  { name: 'Codex', how: 'Same marketplace · codex plugin add' },
  { name: 'Grok', how: 'Claude-compatible hooks' },
  { name: 'Anything HTTP', how: 'POST the open snapshot contract' },
] as const;

/** Main-session lifecycle as the ribbon / panel see it (one job per conversation). */
export const lifecycle = [
  {
    phase: 'Start',
    facet: 'starting',
    ribbon: 'inactive' as const,
    note: 'Session opens — Ready. Running only after the first prompt.',
  },
  {
    phase: 'Work',
    facet: 'thinking · tool',
    ribbon: 'running' as const,
    note: 'Main-thread prompts and tools. Subagents only refine the same row.',
  },
  {
    phase: 'Waiting on you',
    facet: 'idle · permission',
    ribbon: 'attention' as const,
    note: 'Empty background queue + input needed, or an approval prompt.',
  },
  {
    phase: 'Background',
    facet: 'subagent · shell',
    ribbon: 'running' as const,
    note: 'Shell / subagent still running — Running, never Attention.',
  },
  {
    phase: 'Monitor',
    facet: 'monitor',
    ribbon: 'monitor' as const,
    note: 'Monitor open: phase done, watching the stream (purple).',
  },
  {
    phase: 'End',
    facet: 'ended',
    ribbon: 'inactive' as const,
    note: 'SessionEnd removes the job immediately.',
  },
] as const;

export const designNotes = [
  {
    title: 'Jobs, not agents',
    body: 'A job is one unit of work with a producer — Claude, Codex, a build, anything that can POST — while machines stay in Settings as tunnels you configure.',
  },
  {
    title: 'Memory-only runtime',
    body: 'Jobs, timelines, and pending actions never touch disk, so only preferences and a managed ~/.ssh/config block survive a quit.',
  },
  {
    title: 'Fail-open hooks',
    body: 'Agent plugins never block on Nerve, and there are no project env vars because ingest is always loopback :17890.',
  },
  {
    title: 'Out of scope on purpose',
    body: 'Nerve will never reverse-control an agent, sync to a cloud, or keep history on disk, and Open/Focus only jumps you back to the agent UI.',
  },
] as const;
