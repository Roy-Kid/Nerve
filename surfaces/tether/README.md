# Nerve for Tether

A dynamically installed web plugin for macOS Tether: live jobs and a persistent
status ribbon. Install this directory in Tether Settings → Extensions → Install.
Keep `nerve-hub` running separately and allow the plugin's loopback SSE permission.
Public instructions live in the website's `/docs/get-started` page.

Model tests: `node --test testing/tether/model.test.cjs`.
Tether integration test (Tether checkout alongside Nerve):
`swift test --package-path testing/tether`.
