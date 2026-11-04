# AGENTS.md

Agent entry for Codex / Grok / other harnesses. Same contract as [`CLAUDE.md`](./CLAUDE.md).

## Repo map

- **App:** `Nerve/Nerve/` (Swift, display surface)
- **Surfaces:** `surfaces/macos/`, `surfaces/tether/`, `surfaces/tether-web/`, `surfaces/tmux/` — named for their system or host app
- **Tether tests:** `testing/tether/` — model tests and integration with the sibling Tether host
- **VS Code surface:** `vsc-ext/` (rslib/rspack; `cd vsc-ext && npm test`)
- **Plugin:** `plugins/nerve/hooks/` — Claude `nerve.js` (Node exec), Codex `nerve.py` (python3), Grok `grok-post.js` (command POST; Grok `type: http` cannot reach loopback)
- **Hook tests:** `python3 sources/agents/tests/test_nerve_hook.py` · `node --test plugins/nerve/hooks/nerve.test.js`
- **Public docs (website only):** `index/src/docs/content.ts` → `/docs/*`
- **Agent notes:** `.claude/notes/`

No `docs/` directory. Run site docs locally: `cd index && npm run dev`.

## Commands

See [`CLAUDE.md`](./CLAUDE.md#commands): one list, kept in one place.

## Hard rules

- Hub is the only task authority on each machine. Plugins forward raw events; surfaces replace protocol-v2 snapshots and keep UI state only.
- SSH, lifecycle, status, ordering, summaries and notification policy belong exclusively to the hub. `nerve-hub ensure` starts a resident singleton; surfaces never stop it.
- One job per conversation; fail-open hooks; fixed loopback ingest; no free-text status inference.
- Prefer editing site docs content over long repo READMEs.
