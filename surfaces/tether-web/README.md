# Nerve web surface for Tether

`package/` is a runtime web plugin, with no native binaries, build step or
dependency on Tether source. The Windows host installs it under Settings →
Extensions. Product documentation is on the website at `/docs/tether`.

Opening the panel asks the Windows host to start the installed hub if health is offline. Install Nerve to `%LOCALAPPDATA%\Programs\Nerve` first. This surface reads full SSE frames,
reconnects with backoff, and releases its stream when unloaded or disabled.
The page cannot choose executables or arguments: `service.ensure` starts only its host-approved installed companion. It does not write hub state or issue OS notifications.
The label `tether-web` keeps this display-only prototype below notifying
surfaces in the hub's notification election.

```powershell
node --test surfaces/tether-web/tests/model.test.mjs
# With a Windows Tether build containing the web host:
TetherApp.Windows.exe --install-plugin C:\path\nerve\surfaces\tether-web\package --open-plugin app.nerve.tether.web
```
