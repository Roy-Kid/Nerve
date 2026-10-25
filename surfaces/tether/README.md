# Nerve for Tether

On macOS, Tether links the `NervePlugin` product and shows Nerve.app's own
panel (`StatusPanelRoot`), ribbon and settings window. There is no second
job list. The web page in `web/` is what a Tether that does not link that
product can still install from Settings → Extensions.

Keep `nerve-hub` running. It is the same loopback hub the macOS app uses.
Public instructions live in the website's `/docs/get-started` page.

Model tests: `node --test testing/tether/model.test.cjs`.
Tether integration test (Tether checkout alongside Nerve):
`swift test --package-path testing/tether`.
