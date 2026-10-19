import { describe, expect, it } from '@rstest/core';
import { jobs, ribbonWeight, statusMeta } from '../config';
import { buildStatusRibbonGradient } from './statusRibbon';

describe('status ribbon gradient', () => {
  it('uses the same capped transition width as the macOS renderer', () => {
    const gradient = buildStatusRibbonGradient([
      { id: 'running', status: 'running', weight: 1 },
      { id: 'attention', status: 'attention', weight: 1 },
    ]);

    expect(gradient).toBe(
      'linear-gradient(90deg, var(--color-status-running) 0%, var(--color-status-running) 45.5%, var(--color-status-attention) 54.5%, var(--color-status-attention) 100%)',
    );
  });

  it('keeps narrow segments readable with a weight-aware ramp', () => {
    const gradient = buildStatusRibbonGradient([
      { id: 'problem', status: 'problem', weight: 9 },
      { id: 'attention', status: 'attention', weight: 1 },
    ]);

    expect(gradient).toContain('var(--color-status-problem) 87.2%');
    expect(gradient).toContain('var(--color-status-attention) 92.8%');
  });

  it('falls back to the inactive color when no segment is visible', () => {
    expect(buildStatusRibbonGradient([])).toBe(
      'linear-gradient(90deg, var(--color-status-inactive) 0%, var(--color-status-inactive) 100%)',
    );
  });

  it('paints the demo jobs in colour-wheel order with gray last', () => {
    const gradient = buildStatusRibbonGradient(
      jobs.map((job) => ({
        id: job.id,
        status: job.status,
        weight: ribbonWeight(job.status),
      })),
    );

    for (const tone of Object.values(statusMeta)) {
      expect(gradient).toContain(tone.color);
    }
    const green = gradient.indexOf(statusMeta.success.color);
    const blue = gradient.indexOf(statusMeta.running.color);
    const gray = gradient.indexOf(statusMeta.inactive.color);
    expect(green).toBeGreaterThan(-1);
    expect(blue).toBeGreaterThan(green);
    expect(gray).toBeGreaterThan(blue);
  });
});
