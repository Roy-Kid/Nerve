# AGENTS.md

Agent entry for Codex / Grok / other harnesses. Same contract as [`CLAUDE.md`](./CLAUDE.md).

## Repo map

- **App:** `Nerve/Nerve/` (Swift, menu bar + ingest)
- **Windows UI:** `surfaces/windows/` (C# / WinUI 3); presentation and hub connection in Rust `nerve-windows-core`. Legacy egui is an opt-in regression feature, not shipped.
- **VS Code surface:** `vsc-ext/` (rslib/rspack; `cd vsc-ext && npm test`)
- **Tether surface:** `surfaces/tether/` (`swift test --package-path surfaces/tether`)
- **Plugin:** `plugins/nerve/hooks/` — Claude `nerve.js` (Node exec), Codex `nerve.py` (python3), Grok `grok-post.js` (command POST; Grok `type: http` cannot reach loopback)
- **Hook tests:** `python3 sources/agents/tests/test_nerve_hook.py` · `node --test plugins/nerve/hooks/nerve.test.js`
- **Public docs (website only):** `index/src/docs/content.ts` → `/docs/*`
- **Agent notes:** `.claude/notes/`

No `docs/` directory. Run site docs locally: `cd index && npm run dev`.

## Hard rules

- One job per conversation; fail-open hooks; fixed loopback ingest; no free-text status inference.
- Prefer editing site docs content over long repo READMEs.
