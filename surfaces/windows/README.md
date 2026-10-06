# Nerve for Windows

Native C# / WinUI 3 host, consuming a headless Rust presentation binary. The
native title bar/border owns drag and resize; native ListView, buttons and
ToggleSwitch controls own input, focus, accessibility and theme behavior.
Windows Shell tray integration uses .NET's NotifyIcon wrapper. No egui/wgpu
renderer is included in the default Windows build or download.

```
WinUI 3 host ↔ stdin/stdout NDJSON ↔ nerve-windows-core ↔ HTTP/SSE ↔ nerve-hub
```

Rust owns job decoding, derived statuses/colors, grouping/sorting, tooltip and
tray pixels, Ask lease/dedupe, preferences, and navigation/copy rules. C# owns
window lifecycle, native controls, tray registration, Windows notifications,
and login registration. Closing stdin terminates the core and releases its
hub stream; hiding the window keeps the core alive. The host claims the
existing loopback activation port 17891 before starting a core process.

## Build and verify

Windows needs Rust, the .NET 10 SDK, and the Windows SDK/build tools.

```powershell
.\scripts\nerve.ps1 -Build -Run
.\scripts\nerve.ps1 -Test
.\scripts\nerve.ps1 -Install -BinaryDirectory .\target\windows
```

Downloads are self-contained: install the whole package, including DLLs, PRI
resources, language folders, Rust core and hub. Users do not install Rust,
.NET or a Windows App Runtime separately. The package is currently x64.

## Host/core protocol v1

UTF-8, one JSON object per line. Core stdout is reserved for protocol output;
diagnostics go to stderr/log files. Host commands are `cycleGroup`,
`preferences` (with `settings`), `open`/`copy` (with `id`), and `quit`.
Unknown commands return an error; no agent control verbs are supported.

Every output includes `version: 1` and a `type`: `frame`, `action`, `toast`,
or `error`. Frames contain ordered sections/rows, counts, preferences,
tooltip, and a bounded square straight-RGBA bitmap encoded as base64.
Rows include derived status/color, activity/age, prompt/context and recent
timeline. The host rejects incompatible versions and malformed bitmaps.

For isolated native rendering verification, pass `--fixture PATH --smoke PATH`
to the host. It reads the fixture through the Rust core, captures list/detail,
settings, dark and empty XAML renders and records JSON diagnostics, then exits.
It uses an ephemeral activation port, does not attach to the live hub, and does
not write user preferences. This does not substitute for Explorer/mixed-DPI
desktop acceptance.

The old egui host remains available behind Cargo's `legacy-ui` feature only
for migration comparisons. Native hosting and the Rust core are the supported
installation and launch path.
