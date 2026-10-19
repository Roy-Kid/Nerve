import { jobs, statusMeta, type StatusTone } from '../config';
import { cn } from '../lib/utils';
import { useLocale } from '../i18n/locale';

const selectedId = jobs.find((job) => job.status === 'attention')?.id ?? jobs[0].id;

const STATUS_GLYPH: Record<StatusTone, string> = {
  problem: '✕',
  attention: '◐',
  running: '●',
  monitor: '◎',
  success: '◎',
  inactive: '○',
};

function tmuxFilterChips(statuses: readonly StatusTone[]) {
  const n = (status: StatusTone) => statuses.filter((item) => item === status).length;
  return [
    { icon: '≡', count: statuses.length, color: '#f5f5f7', active: true },
    { icon: '●', count: n('running'), color: statusMeta.running.color, active: false },
    { icon: '◎', count: n('monitor') + n('success'), color: statusMeta.monitor.color, active: false },
    { icon: '◐', count: n('attention'), color: statusMeta.attention.color, active: false },
    { icon: '○', count: n('inactive'), color: statusMeta.inactive.color, active: false },
    { icon: '✕', count: n('problem'), color: statusMeta.problem.color, active: false },
  ];
}

type TmuxSidebarPreviewProps = {
  variant?: 'default' | 'hero' | 'card';
  className?: string;
};

export function TmuxSidebarPreview({
  variant = 'default',
  className,
}: TmuxSidebarPreviewProps) {
  const { t } = useLocale();
  const hero = variant === 'hero';
  const rows = jobs.map((job, index) => {
    const copy = t.preview.tmux.jobs[index];
    return {
      ...job,
      name: copy?.name ?? job.label,
      detail: copy?.detail ?? t.preview.status[job.status],
      time: copy?.time ?? '',
      active: job.id === selectedId,
    };
  });
  const selected = rows.find((job) => job.active) ?? rows[0];
  const chips = tmuxFilterChips(jobs.map((job) => job.status));

  return (
    <div
      className={cn(
        'flex aspect-[16/10] w-full flex-col overflow-hidden rounded-2xl border border-black/20 bg-[#1c1c1e] font-mono text-[#d0d0d0] shadow-[0_28px_64px_rgb(0_0_0/28%)]',
        hero && 'rounded-[20px] shadow-[0_38px_84px_rgb(0_0_0/27%)]',
        className,
      )}
      aria-label={t.preview.tmux.label}
    >
      <div
        className={cn(
          'flex h-7 shrink-0 items-center gap-1.5 border-b border-white/10 bg-[#2c2c2e] px-2.5 text-[8px] text-[#a1a1a6]',
          hero && 'h-9 px-3.5 text-[11px]',
        )}
        aria-hidden="true"
      >
        <span className={cn('rounded-full bg-traffic-red', hero ? 'size-2.5' : 'size-[7px]')} />
        <span className={cn('rounded-full bg-traffic-amber', hero ? 'size-2.5' : 'size-[7px]')} />
        <span className={cn('rounded-full bg-traffic-green', hero ? 'size-2.5' : 'size-[7px]')} />
        <span className="ml-1">nerve</span>
        <span className="opacity-50">— tmux</span>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,38%)_minmax(0,1fr)]">
        <aside className="flex min-w-0 flex-col bg-[#111111] text-[#d9d9d9]">
          <div
            className={cn(
              'flex h-6 shrink-0 items-center gap-1.5 px-2 text-[8px] tracking-wide',
              hero && 'h-8 px-2.5 text-[12px]',
            )}
            aria-hidden="true"
          >
            {chips.map((chip) => (
              <b
                key={chip.icon}
                className={cn('font-semibold', chip.active && 'underline')}
                style={{ color: chip.color }}
              >
                {chip.icon}
                {chip.count}
              </b>
            ))}
          </div>

          <ul className="m-0 min-h-0 flex-1 list-none overflow-hidden">
            {rows.map((job) => (
              <li
                key={job.id}
                className={cn(
                  'grid grid-cols-[1.1rem_minmax(0,1fr)_auto] items-center gap-1 px-2 font-medium',
                  hero ? 'h-8 text-[12px]' : 'h-[18px] text-[8px]',
                  job.active ? 'bg-[#2a2a2a] text-white' : 'text-[#c8c8c8]',
                )}
              >
                <span style={{ color: statusMeta[job.status].color }}>
                  {STATUS_GLYPH[job.status]}
                </span>
                <strong className="overflow-hidden font-semibold text-ellipsis whitespace-nowrap">
                  {job.name}
                </strong>
                <time className="opacity-70">{job.time}</time>
              </li>
            ))}
          </ul>

          <div
            className={cn(
              'shrink-0 border-t border-white/10 px-2 py-1.5',
              hero && 'px-2.5 py-2',
            )}
          >
            <div
              className={cn(
                'text-[7px] font-semibold tracking-[0.08em] text-[#8e8e93] uppercase',
                hero && 'text-[10px]',
              )}
            >
              Prompt
            </div>
            <p
              className={cn(
                'm-0 mt-0.5 overflow-hidden text-[8px] text-[#e8e8e8] text-ellipsis whitespace-nowrap',
                hero && 'text-[12px]',
              )}
            >
              {t.preview.tmux.prompt}
            </p>
          </div>
        </aside>

        <div
          className={cn(
            'relative min-w-0 overflow-hidden bg-[#0e0e0e] px-2.5 pt-2 font-mono text-[#8a8a8a]',
            hero ? 'text-[11px] leading-5' : 'text-[7px] leading-4',
          )}
          aria-hidden="true"
        >
          <p className="m-0 text-[#6e6e73]">~/nerve</p>
          <p className="m-0 mt-1">
            <span className="text-[#30d158]">❯</span> claude
          </p>
          <p className="m-0 mt-2 text-status-attention">{t.preview.status.attention}</p>
          <p className="m-0 mt-1 text-[#d0d0d0]">{selected.name}</p>
          <p className="m-0 mt-1 text-[#636366]">{selected.detail}</p>
        </div>
      </div>

      <div
        className={cn(
          'flex h-[18px] shrink-0 items-center gap-2 bg-[#1eb500] px-2 text-[7px] font-semibold text-[#061806]',
          hero && 'h-6 text-[11px]',
        )}
      >
        <b>[nerve]</b>
        <span className="font-normal opacity-80">0:zsh- 1:claude*</span>
        <em className="ml-auto font-normal not-italic">"nerve"</em>
      </div>
    </div>
  );
}
