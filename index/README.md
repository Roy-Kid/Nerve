# Nerve website

Marketing landing + **product handbook** (SPA under `/docs/*`).

## Commands

```bash
npm install
npm run dev      # http://localhost:3000
npm run build    # → dist/
npm test
```

## Routes

| Path | Page |
|------|------|
| `/` | Home (hero, signal, macOS, tmux, VS Code, docs) |
| `/docs` | Docs hub |
| `/docs/get-started` | Get started |
| `/docs/plugin` | Agent plugins |
| `/docs/machines` | Machines & remotes |
| `/docs/status` | Status & lifecycle |
| `/docs/ingest` | Ingest API |
| `/docs/tmux` | tmux surface |
| `/docs/vscode` | VS Code surface |
| `/docs/windows` | Windows surface |
| `/docs/privacy` | Privacy |

Handbook body: [`src/docs/content.ts`](./src/docs/content.ts).  
UI: `src/pages/` · `src/components/docs/`.

This is the **only** public docs surface for the monorepo (no top-level `docs/`).

## Deploy

`npm run build` → `dist/`. Hosts need SPA fallback (`/* → index.html`); `public/_redirects` covers Netlify/Cloudflare-style. Product URLs: `src/config.ts`.
