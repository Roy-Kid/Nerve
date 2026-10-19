import { motion } from 'motion/react';
import { Link } from 'react-router-dom';
import { site } from '../../config';
import { usePrefersReducedMotion } from '../../hooks/usePrefersReducedMotion';
import { useLocale } from '../../i18n/locale';
import { ease } from '../../lib/motion';
import { homeLinkChevron, sectionBody, sectionTitle } from '../../lib/ui';

const protocolLink =
  'group inline-flex w-fit items-baseline text-[17px] font-[500] text-night-link hover:underline hover:underline-offset-4 focus-visible:rounded focus-visible:outline-[3px] focus-visible:outline-offset-4 focus-visible:outline-blue/40';

function FlowNode({ label, detail }: { label: string; detail: string }) {
  return (
    <div className="rounded-[16px] border border-white/12 bg-white/[0.055] px-5 py-4 shadow-[0_18px_48px_rgb(0_0_0/18%)] max-sm:rounded-xl max-sm:px-3 max-sm:py-2">
      <strong className="block text-[15px] font-[650] text-night-text">{label}</strong>
      <span className="mt-1 block text-[12px] leading-[1.45] text-night-muted">{detail}</span>
    </div>
  );
}

function FlowArrow({ endpoint }: { endpoint: string }) {
  return (
    <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-3 py-2 font-mono text-[10px] text-night-link max-sm:py-1">
      <span className="h-px bg-white/12" />
      <code>{endpoint}</code>
      <span className="relative h-px bg-white/12 after:absolute after:top-1/2 after:right-0 after:size-1.5 after:-translate-y-1/2 after:rotate-45 after:border-t after:border-r after:border-night-link" />
    </div>
  );
}

export function HubDocsSection() {
  const { t } = useLocale();
  const reduced = usePrefersReducedMotion();

  return (
    <section
      id="open"
      className="flex h-svh scroll-mt-12 items-center overflow-hidden bg-night bg-[radial-gradient(circle_at_18%_82%,rgb(41_121_255/19%),transparent_32rem)] px-[var(--page-gutter)] py-[clamp(58px,8vh,92px)] text-night-text max-sm:py-9"
      aria-labelledby="home-open-title"
    >
      <div className="mx-auto grid w-full max-w-page grid-cols-[minmax(0,0.94fr)_minmax(340px,1.06fr)] items-center gap-[clamp(52px,8vw,110px)] max-lap:gap-8 max-sm:grid-cols-1 max-sm:gap-[clamp(16px,3vh,26px)]">
        <motion.header
          className="max-w-[560px]"
          initial={reduced ? false : { opacity: 0, y: 24 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true, margin: '-15%' }}
          transition={ease(0.65)}
        >
          <p className="m-0 mb-4 text-sm/[1.5] font-[650] text-night-link">
            {t.hub.open.kicker}
          </p>
          <h2 id="home-open-title" className={sectionTitle}>
            <span className="block">{t.hub.open.title[0]}</span>
            <span className="mt-[0.08em] block text-night-muted">{t.hub.open.title[1]}</span>
          </h2>
          <p className={`${sectionBody} text-night-muted`}>{t.hub.open.body}</p>
          <div className="mt-7 flex flex-wrap gap-x-7 gap-y-3">
            <Link to="/docs/ingest" className={protocolLink}>
              {t.hub.open.protocolCta}
              <span aria-hidden="true" className={homeLinkChevron}>
                {' '}
                ›
              </span>
            </Link>
            <a href={site.github} target="_blank" rel="noreferrer" className={protocolLink}>
              {t.hub.open.sourceCta}
              <span aria-hidden="true" className={homeLinkChevron}>
                {' '}
                ↗
              </span>
            </a>
          </div>
        </motion.header>

        <motion.figure
          className="m-0 rounded-[22px] border border-white/12 bg-night p-[clamp(18px,3vw,30px)] shadow-night max-sm:rounded-2xl max-sm:p-3"
          initial={reduced ? false : { opacity: 0, y: 34, scale: 0.97 }}
          whileInView={{ opacity: 1, y: 0, scale: 1 }}
          viewport={{ once: true, margin: '-10%' }}
          transition={ease(0.85)}
          aria-label="Nerve open protocol flow"
        >
          <FlowNode label={t.hub.open.producers} detail={t.hub.open.producerRole} />
          <FlowArrow endpoint="POST /v1/snapshot" />
          <div className="rounded-[18px] border border-blue/55 bg-blue/12 px-5 py-5 shadow-[0_0_42px_rgb(0_122_255/16%)] max-sm:rounded-xl max-sm:px-3 max-sm:py-3">
            <div className="flex items-center gap-2 font-mono text-[11px] text-night-link">
              <span className="size-2 rounded-full bg-status-running shadow-[0_0_12px_var(--color-status-running)]" />
              nerve-hub · 127.0.0.1:17890
            </div>
            <strong className="mt-2 block text-[16px] font-[650] text-night-text">
              {t.hub.open.hub}
            </strong>
          </div>
          <FlowArrow endpoint="GET /v1/stream · SSE" />
          <FlowNode label={t.hub.open.surfaces} detail={t.hub.open.surfaceRole} />
        </motion.figure>
      </div>
    </section>
  );
}
