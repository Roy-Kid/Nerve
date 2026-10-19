import { describe, expect, it } from '@rstest/core';
import {
  designNotes,
  jobs,
  lifecycle,
  ribbonWeight,
  site,
  sources,
  statusMeta,
  truths,
} from './config';
import { messages } from './i18n/messages';

describe('site config', () => {
  it('exposes product name and GitHub URL', () => {
    expect(site.name).toBe('Nerve');
    expect(site.github).toMatch(/^https:\/\/github\.com\//);
  });

  it('exposes an App Store URL', () => {
    expect(site.appStore).toMatch(/^https:\/\/apps\.apple\.com\//);
  });

  it('has a two-line headline', () => {
    expect(site.headline).toHaveLength(2);
  });

  it('demo jobs follow the colour wheel, with gray last and half-weight', () => {
    expect(jobs.map((job) => job.status)).toEqual([
      'problem',
      'attention',
      'success',
      'running',
      'monitor',
      'inactive',
    ]);
    expect(ribbonWeight('inactive')).toBe(0.5);
    expect(ribbonWeight('running')).toBe(1);
    expect(jobs.at(-1)?.status).toBe('inactive');
  });

  it('mac, tmux and vscode screenshot rows follow the same demo jobs', () => {
    for (const locale of ['en', 'zh'] as const) {
      expect(messages[locale].preview.mac.jobs).toHaveLength(jobs.length);
      expect(messages[locale].preview.tmux.jobs).toHaveLength(jobs.length);
      expect(messages[locale].preview.vscode.jobs).toHaveLength(jobs.length);
    }
  });

  it('covers the status palette', () => {
    expect(Object.keys(statusMeta)).toEqual([
      'problem',
      'attention',
      'running',
      'monitor',
      'success',
      'inactive',
    ]);
    expect(statusMeta.running.label).toBe('Running');
    expect(statusMeta.attention.label).toBe('Needs you');
    expect(statusMeta.success.label).toBe('Completed');
    expect(statusMeta.inactive.label).toBe('Ready / idle');
    expect(statusMeta.problem.color).toBe('var(--color-status-problem)');
    expect(statusMeta.attention.color).toBe('var(--color-status-attention)');
  });

  it('lists product truths and sources', () => {
    expect(truths).toHaveLength(3);
    expect(sources.length).toBeGreaterThanOrEqual(3);
  });

  it('defines main-session lifecycle for the Signal section', () => {
    expect(lifecycle.length).toBeGreaterThanOrEqual(4);
    expect(designNotes.length).toBeGreaterThanOrEqual(3);
    expect(lifecycle.some((s) => s.phase === 'End')).toBe(true);
  });

  it('points docs links at in-site subpages', () => {
    expect(site.docs).toBe('/docs');
    expect(site.pluginDocs).toBe('/docs/plugin');
    expect(site.macos).toBe('/#macos');
    expect(site.tmux).toBe('/#tmux');
    expect(site.vscode).toBe('/#vscode');
  });
});
