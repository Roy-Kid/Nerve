# nerve

Marketplace plugin: push Claude Code / Codex / Grok **session lifecycle** into nerve-hub (`http://127.0.0.1:17890`).

Each host uses **its official native hook type and language**:

| Host | Official type | Native script |
|------|---------------|---------------|
| Claude Code | `command` exec form (`node` + `args`) | `hooks/nerve.js` |
| Codex | `command` (`python3 ${PLUGIN_ROOT}/…`) | `hooks/nerve.py` |
| Grok | `command` | `hooks/grok-post.js` POSTs stdin to `/v1/hook`. Grok `type: http` blocks loopback (SSRF). |

All three entry points only collect source metadata and forward raw events to `/v1/hook?producer=…`. Event mapping and task state live in the main hub crate. Multiple hook registrations correspond to different host lifecycle events, not independent task implementations.

Fail-open: bounded network time, always exit 0. Start the local authority with `nerve-hub ensure` before using a surface.

## Docs

→ Website `/docs/plugin` (`index/src/docs/content.ts`).

## Install

```text
# Claude Code
/plugin marketplace add Roy-Kid/nerve
/plugin install nerve@nerve
```

```bash
codex plugin marketplace add Roy-Kid/nerve
codex plugin add nerve@nerve
```

## Develop

```bash
python3 sources/agents/tests/test_nerve_hook.py
node --test plugins/nerve/hooks/nerve.test.js
cargo test -p nerve-hub hook   # all producer mappings in the hub
python3 -B scripts/check-plugin.py
```

Local Codex desktop import: select this `plugins/nerve` folder (the entry manifest is `.codex-plugin/plugin.json`, pointing to `hooks/codex.json`). Enable the plugin and trust its hooks in Codex. Do not import the repository root. Run the package check before importing: Python bytecode and cache directories are forbidden.
