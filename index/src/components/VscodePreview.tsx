import { jobs, statusMeta } from '../config';
import { cn } from '../lib/utils';
import { useLocale } from '../i18n/locale';

const selectedId = jobs.find((job) => job.status === 'attention')?.id ?? jobs[0].id;
const attentionCount = jobs.filter(
  (job) => job.status === 'attention' || job.status === 'problem',
).length;

type VscodePreviewProps = {
  variant?: 'default' | 'hero' | 'card';
  className?: string;
};

export function VscodePreview({ variant = 'default', className }: VscodePreviewProps) {
  const { t } = useLocale();
  const hero = variant === 'hero';
  const rows = jobs.map((job, index) => {
    const copy = t.preview.vscode.jobs[index];
    return {
      ...job,
      name: copy?.name ?? job.label,
      detail: copy?.detail ?? t.preview.status[job.status],
      active: job.id === selectedId,
    };
  });

  return (
    <div
      className={cn(
        'flex aspect-[16/10] w-full flex-col overflow-hidden rounded-[10px] border border-[#2b2b2b] bg-[#1e1e1e] text-[#cccccc] shadow-[0_28px_64px_rgb(0_0_0/28%)]',
        hero && 'rounded-[14px] shadow-[0_38px_84px_rgb(0_0_0/27%)]',
        className,
      )}
      aria-label={t.preview.vscode.label}
    >
      <div
        className={cn(
          'flex h-8 shrink-0 items-center border-b border-black/40 bg-[#3c3c3c] px-2 text-[10px] text-[#cccccc]',
          hero && 'h-10 px-3 text-[12px]',
        )}
        aria-hidden="true"
      >
        <span className="flex items-center gap-1.5">
          <span className={cn('rounded-full bg-traffic-red', hero ? 'size-2.5' : 'size-2')} />
          <span className={cn('rounded-full bg-traffic-amber', hero ? 'size-2.5' : 'size-2')} />
          <span className={cn('rounded-full bg-traffic-green', hero ? 'size-2.5' : 'size-2')} />
        </span>
        <span className="mx-auto rounded-md bg-[#4d4d4d] px-8 py-0.5 text-[#cccccc]/80">
          nerve
        </span>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[36px_minmax(0,34%)_minmax(0,1fr)] max-sm:grid-cols-[28px_minmax(0,1fr)]">
        <nav
          className="flex flex-col items-center gap-1 border-r border-black/50 bg-[#333333] pt-1"
          aria-hidden="true"
        >
          <span className="relative flex h-8 w-full items-center justify-center bg-white/6">
            <i className="absolute inset-y-1 left-0 w-0.5 rounded-r bg-white" />
            <span className="text-[11px] font-semibold tracking-tight text-white">N</span>
            {attentionCount > 0 ? (
              <b className="absolute top-0.5 right-0.5 flex size-3 items-center justify-center rounded-full bg-[#007acc] text-[7px] font-bold text-white">
                {attentionCount}
              </b>
            ) : null}
          </span>
          <span className="mt-1 size-3.5 rounded-[2px] border border-white/25" />
          <span className="size-3.5 rounded-full border border-white/25" />
          <span className="size-3.5 rounded-[2px] border-y border-white/25" />
          <span className="mt-auto mb-2 size-3.5 rounded-full border border-white/20" />
        </nav>

        <aside className="flex min-w-0 flex-col bg-[#252526]">
          <div
            className={cn(
              'flex h-8 shrink-0 items-center justify-between px-3 text-[10px] font-semibold tracking-[0.14em] text-[#bbbbbb] uppercase',
              hero && 'text-[11px]',
            )}
          >
            Nerve
            <span className="tracking-normal text-[#888] lowercase">···</span>
          </div>
          <div
            className={cn(
              'flex h-6 items-center gap-1 px-3 text-[10px] text-[#cccccc]',
              hero && 'text-[11px]',
            )}
          >
            <span className="text-[8px] opacity-70">▾</span>
            {t.preview.vscode.sidebar}
          </div>
          <ul className="m-0 min-h-0 flex-1 list-none overflow-hidden pb-1">
            {rows.map((job) => (
              <li
                key={job.id}
                className={cn(
                  'flex h-[22px] items-center gap-2 px-3 text-[11px]',
                  hero && 'h-9 text-[12px]',
                  job.active ? 'bg-[#04395e] text-white' : 'text-[#cccccc]',
                )}
              >
                <i
                  className="size-[7px] shrink-0 rounded-full"
                  style={{ background: statusMeta[job.status].color }}
                  aria-hidden="true"
                />
                <span className="min-w-0 overflow-hidden">
                  <strong className="block overflow-hidden font-medium text-ellipsis whitespace-nowrap">
                    {job.name}
                  </strong>
                  {hero ? (
                    <small className="block overflow-hidden text-[9px] leading-3 text-ellipsis whitespace-nowrap opacity-60">
                      {job.detail}
                    </small>
                  ) : null}
                </span>
              </li>
            ))}
          </ul>
        </aside>

        <div className="flex min-w-0 flex-col bg-[#1e1e1e] max-sm:hidden" aria-hidden="true">
          <div className="flex h-8 shrink-0 items-end border-b border-black/40 bg-[#252526] text-[11px]">
            <span className="flex h-[26px] items-center border-t-2 border-t-[#007acc] bg-[#1e1e1e] px-3 text-[#ffffff]">
              {t.preview.vscode.editor}
            </span>
            <span className="flex h-[26px] items-center px-3 text-[#969696]">README.md</span>
          </div>
          <div
            className={cn(
              'grid flex-1 grid-cols-[28px_minmax(0,1fr)] font-mono text-[#d4d4d4]',
              hero ? 'text-[12px] leading-6' : 'text-[10px] leading-5',
            )}
          >
            <div className="bg-[#1e1e1e] pt-2 pr-2 text-right text-[#858585]">
              <div>1</div>
              <div>2</div>
              <div>3</div>
            </div>
            <div className="pt-2">
              <div>
                <span className="text-[#c586c0]">const</span>
                {' '}
                <span className="text-[#9cdcfe]">status</span>
                {' = '}
                <span className="text-[#ce9178]">'{t.preview.status.attention}'</span>
              </div>
              <div className="text-[#6a9955]">// Nerve</div>
            </div>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'flex h-[22px] shrink-0 items-center bg-[#007acc] px-2 text-[10px] text-white',
          hero && 'h-6 text-[11px]',
        )}
      >
        <span className="opacity-90">main*</span>
        <span className="ml-3 opacity-70">0</span>
        <span className="ml-auto rounded-sm bg-[#cca700] px-2 py-px font-semibold text-[#1d1d1f]">
          Nerve {attentionCount}↑
        </span>
        <span className="ml-3 opacity-80">UTF-8</span>
      </div>
    </div>
  );
}
