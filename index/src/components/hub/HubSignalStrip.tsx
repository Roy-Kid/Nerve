import { motion } from 'motion/react';
import { jobs, statusMeta } from '../../config';
import { usePrefersReducedMotion } from '../../hooks/usePrefersReducedMotion';
import { useLocale } from '../../i18n/locale';
import { ease } from '../../lib/motion';
import { sectionBody, sectionTitle } from '../../lib/ui';

export function HubSignalStrip() {
  const { t } = useLocale();
  const reduced = usePrefersReducedMotion();
  const rows = jobs.map((job, index) => ({
    ...job,
    ...t.preview.mac.jobs[index],
    statusLabel: t.preview.status[job.status],
  }));

  return (
    <section
      id="product"
      className="flex h-svh scroll-mt-12 items-center overflow-hidden bg-night bg-[radial-gradient(circle_at_78%_22%,rgb(66_92_164/25%),transparent_34rem)] px-[var(--page-gutter)] py-[clamp(58px,8vh,92px)] text-night-text max-sm:py-11"
      aria-labelledby="home-product-title"
    >
      <div className="mx-auto grid w-full max-w-page grid-cols-[minmax(0,0.92fr)_minmax(340px,1.08fr)] items-center gap-[clamp(48px,8vw,104px)] max-lap:gap-8 max-sm:grid-cols-1 max-sm:gap-[clamp(18px,3vh,28px)]">
        <motion.header
          className="max-w-[520px]"
          initial={reduced ? false : { opacity: 0, y: 24 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true, margin: '-15%' }}
          transition={ease(0.65)}
        >
          <p className="m-0 mb-4 text-sm/[1.5] font-[650] text-night-link">
            {t.hub.product.kicker}
          </p>
          <h2 id="home-product-title" className={sectionTitle}>
            <span className="block">{t.hub.product.title[0]}</span>
            <span className="mt-[0.08em] block text-night-muted">{t.hub.product.title[1]}</span>
          </h2>
          <p className={`${sectionBody} text-night-muted`}>{t.hub.product.body}</p>

          <dl className="mt-[clamp(24px,4vh,38px)] grid gap-0 border-t border-white/12">
            {t.hub.product.facts.map((fact) => (
              <div
                key={fact.title}
                className="grid grid-cols-[minmax(120px,0.62fr)_minmax(0,1fr)] gap-5 border-b border-white/10 py-3.5 max-sm:grid-cols-1 max-sm:gap-1 max-sm:py-2.5"
              >
                <dt className="text-[14px] font-[650] text-night-text">{fact.title}</dt>
                <dd className="m-0 text-[13px] leading-[1.45] text-night-muted max-sm:hidden">
                  {fact.body}
                </dd>
              </div>
            ))}
          </dl>
        </motion.header>

        <motion.figure
          className="m-0 min-w-0 overflow-hidden rounded-[22px] border border-white/12 bg-night shadow-night"
          initial={reduced ? false : { opacity: 0, y: 34, scale: 0.97 }}
          whileInView={{ opacity: 1, y: 0, scale: 1 }}
          viewport={{ once: true, margin: '-10%' }}
          transition={ease(0.85)}
        >
          <figcaption className="flex h-11 items-center border-b border-white/10 bg-white/[0.035] px-4 font-mono text-[11px] text-night-muted max-sm:h-9 max-sm:px-3 max-sm:text-[9px]">
            <span className="mr-2 size-2 rounded-full bg-status-running shadow-[0_0_12px_var(--color-status-running)]" />
            nerve-hub · 127.0.0.1:17890
            <span className="ml-auto">{t.hub.product.live}</span>
          </figcaption>

          <ul className="m-0 list-none p-0">
            {rows.map((job) => (
              <li
                key={job.id}
                className="grid min-h-[54px] grid-cols-[10px_minmax(100px,0.78fr)_minmax(110px,1fr)_auto] items-center gap-3 border-b border-white/[0.075] px-4 last:border-b-0 max-sm:min-h-9 max-sm:grid-cols-[8px_minmax(0,1fr)_auto] max-sm:gap-2 max-sm:px-3 max-sm:[&:nth-child(n+5)]:hidden"
              >
                <i
                  aria-hidden="true"
                  className="size-2 rounded-full"
                  style={{
                    backgroundColor: statusMeta[job.status].color,
                    boxShadow: `0 0 12px color-mix(in srgb, ${statusMeta[job.status].color} 40%, transparent)`,
                  }}
                />
                <span className="min-w-0">
                  <strong className="block overflow-hidden text-[13px] font-[620] text-ellipsis whitespace-nowrap text-night-text max-sm:text-[11px]">
                    {job.name}
                  </strong>
                  <small className="mt-0.5 block font-mono text-[9px] text-night-muted max-sm:hidden">
                    {job.alias}
                  </small>
                </span>
                <span className="overflow-hidden text-[12px] text-ellipsis whitespace-nowrap text-night-muted max-sm:hidden">
                  {job.detail}
                </span>
                <span className="flex items-center gap-3 font-mono text-[10px] whitespace-nowrap text-night-muted max-sm:text-[9px]">
                  <b className="font-[600]" style={{ color: statusMeta[job.status].color }}>
                    {job.statusLabel}
                  </b>
                  <time className="max-sm:hidden">{job.time}</time>
                </span>
              </li>
            ))}
          </ul>
        </motion.figure>
      </div>
    </section>
  );
}
