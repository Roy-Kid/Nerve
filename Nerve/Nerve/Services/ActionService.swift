import Foundation
import AppKit

enum ActionResult: Sendable {
    case succeeded(String)
    case failed(String)
    case pending(String)   // queued for owning producer
    case unsupported
    case denied(String)
}

/// Local display helpers only. Nerve does **not** reverse-control agents or jobs.
///
/// Primary local action is **Open / Focus** (jump to agent UI / workspace).
/// Remote kinds (approve / cancel / submit_input / …) are rejected — status is push-only.
enum ActionService {
    /// Kinds Nerve can fulfill locally without the source process.
    static let localKinds: Set<String> = [
        "open", "focus", "open_url", "openurl",
        "copy", "copy_summary", "copysummary",
        "open_logs", "openlogs", "hide", "mute",
    ]

    /// Former remote control kinds — kept for classification / UI filtering only.
    /// Nerve never enqueues these to a producer.
    static let remoteKinds: Set<String> = [
        "approve", "reject", "cancel", "pause", "resume", "retry", "submit",
        "submit_input", "submitinput",
    ]

    /// Open / Focus kinds — jump back to agent workspace / IDE / folder.
    static let openKinds: Set<String> = [
        "open", "focus", "open_url", "openurl",
    ]

    /// Local helpers that should stay re-usable after a successful click.
    static let reusableKinds: Set<String> = [
        "open", "focus", "open_url", "openurl",
        "copy", "copy_summary", "copysummary",
        "open_logs", "openlogs",
    ]

    /// True when the action is display-local (copy, open, dismiss, …).
    static func isDisplayAction(_ action: JobAction) -> Bool {
        let k = action.kind.lowercased()
        if localKinds.contains(k) { return true }
        // Unknown custom control kinds are not display-safe.
        if remoteKinds.contains(k) { return false }
        if action.kind.hasPrefix("custom.") { return false }
        return false
    }

    static func isOpenKind(_ kind: String) -> Bool {
        openKinds.contains(kind.lowercased())
    }

    /// Whether this job can jump somewhere (URL or focus breadcrumb).
    static func canFocus(_ subject: Job) -> Bool {
        if let url = subject.location?.openURL, !url.isEmpty { return true }
        if let hint = subject.location?.focusHint, !hint.isEmpty { return true }
        return false
    }

    /// Preferred primary local action title for the panel.
    ///
    /// A job on another machine says so on the button: clicking it opens a
    /// terminal *there*, which is a bigger thing than revealing a folder here
    /// and must not be labelled the same way.
    static func focusActionTitle(for subject: Job) -> String {
        if let alias = RemoteMachine.foreignAlias(of: subject.alias) {
            if ideDeepLink(subject) != nil { return "Open on \(alias)" }
            return remoteSSHHost(for: subject) != nil ? "Connect to \(alias)" : "Copy location"
        }
        if let url = subject.location?.openURL, !url.isEmpty {
            return url.hasPrefix("file:") || url.hasPrefix("/") ? "Open workspace" : "Open agent"
        }
        if let path = focusPath(subject), FileManager.default.fileExists(atPath: path) { return "Open workspace" }
        return "Copy location"
    }

    @MainActor
    static func classify(action: JobAction) -> KindClass {
        let k = action.kind.lowercased()
        if localKinds.contains(k) { return .local }
        // Display-only product: never treat as remote enqueue.
        if remoteKinds.contains(k) { return .unsupportedRemote }
        if action.kind.hasPrefix("custom.") { return .unsupportedRemote }
        return .localIfOpenURLElseRemote
    }

    enum KindClass {
        case local
        /// Would have been remote control — Nerve refuses (display-only).
        case unsupportedRemote
        case localIfOpenURLElseRemote
    }

    @MainActor
    static func performLocal(action: JobAction, on subject: Job) -> ActionResult {
        guard action.state == .available || action.state == .pending else {
            return .denied("Action is \(action.state.rawValue)")
        }

        switch action.kind.lowercased() {
        case "open", "focus", "open_url", "openurl":
            return openLocation(subject)

        case "copy", "copy_summary", "copysummary":
            let detail = subject.current?.summary ?? subject.current?.name ?? subject.attention.title
            let text = detail.map { "\(subject.name) — \($0)" } ?? subject.name
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            return .succeeded("Copied summary")

        case "open_logs", "openlogs":
            if let path = subject.location?.logPath {
                let url = URL(fileURLWithPath: path)
                if FileManager.default.fileExists(atPath: path) {
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                    return .succeeded("Revealed logs")
                }
                return .failed("Log path not found")
            }
            return .failed("No log path")

        case "hide", "mute":
            return .succeeded("Acknowledged (\(action.kind))")

        default:
            if subject.location?.openURL != nil || subject.location?.focusHint != nil {
                return openLocation(subject)
            }
            return .unsupported
        }
    }

    private static func focusPath(_ subject: Job) -> String? {
        guard let hint = subject.location?.focusHint?.trimmingCharacters(in: .whitespacesAndNewlines) else { return nil }
        if hint.hasPrefix("/") { return hint }
        if let range = hint.range(of: " · /") { return String(hint[range.lowerBound...].dropFirst(3)) }
        return nil
    }

    /// Open workspace / IDE deep link, else copy focus breadcrumb for the user.
    ///
    /// A job on another machine is answered by [`openRemote`] instead: its
    /// `openURL` names that machine's filesystem, and opening it here would at
    /// best fail and at worst open an unrelated local folder of the same name.
    @MainActor
    static func openLocation(_ subject: Job) -> ActionResult {
        if let alias = RemoteMachine.foreignAlias(of: subject.alias) {
            return openRemote(subject, alias: alias)
        }
        if let raw = subject.location?.openURL?.trimmingCharacters(in: .whitespacesAndNewlines),
           !raw.isEmpty {
            if let url = URL(string: raw) {
                let ok = NSWorkspace.shared.open(url)
                if ok { return .succeeded("Opened") }
            }
            // Malformed scheme string — try as filesystem path.
            if raw.hasPrefix("/") {
                let url = URL(fileURLWithPath: raw)
                let ok = NSWorkspace.shared.open(url)
                if ok { return .succeeded("Opened") }
            }
            return .failed("Could not open URL")
        }

        if let hint = subject.location?.focusHint?.trimmingCharacters(in: .whitespacesAndNewlines),
           !hint.isEmpty {
            // If the hint is (or ends with) an absolute path, open that folder.
            let pathCandidate = focusPath(subject)
            if let path = pathCandidate, FileManager.default.fileExists(atPath: path) {
                let ok = NSWorkspace.shared.open(URL(fileURLWithPath: path))
                if ok { return .succeeded("Opened workspace") }
            }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(hint, forType: .string)
            return .succeeded("Copied focus hint")
        }
        return .failed("No open target")
    }

    /// Take the human to a job running on another machine.
    ///
    /// Same order as the tmux surface (`crates/nerve-tmux-surface/src/preview.rs`):
    /// an IDE deep link routes itself to its own remote, otherwise the ssh
    /// session that reaches that machine, otherwise nothing local can do it and
    /// the breadcrumb is copied instead. A remote path is never handed to
    /// NSWorkspace — display only means honest, not eager.
    @MainActor
    private static func openRemote(_ subject: Job, alias: String) -> ActionResult {
        if let url = ideDeepLink(subject), NSWorkspace.shared.open(url) {
            return .succeeded("Opened")
        }
        if let host = remoteSSHHost(for: subject) {
            guard let url = URL(string: "ssh://\(host)") else {
                return .failed("Could not address ssh host \(host)")
            }
            // Whatever the user registered for `ssh:` — Terminal by default.
            if NSWorkspace.shared.open(url) { return .succeeded("Opened ssh \(host)") }
            return .failed("No app to open ssh \(host)")
        }
        return copyFocusBreadcrumb(subject, fallback: "\(alias) — no ssh host in ~/.ssh/config")
    }

    /// `openURL` when it is a link its app resolves on its own (`cursor://`,
    /// `vscode://`). A `file://` URL is a path on the reporting machine and is
    /// never one of these.
    static func ideDeepLink(_ subject: Job) -> URL? {
        guard let raw = subject.location?.openURL?.trimmingCharacters(in: .whitespacesAndNewlines),
              !raw.isEmpty,
              let url = URL(string: raw),
              let scheme = url.scheme?.lowercased(),
              scheme != "file"
        else { return nil }
        return url
    }

    /// ssh `Host` that reaches `alias`, from the user's own config — the same
    /// file Settings sources its Machines list from (CLAUDE.md invariant 5).
    private static func remoteSSHHost(for subject: Job) -> String? {
        guard case .string(let host) = subject.extensions["sshHost"] else { return nil }
        return host
    }

    /// Last resort: hand the human the breadcrumb so they can get there
    /// themselves.
    @MainActor
    private static func copyFocusBreadcrumb(_ subject: Job, fallback: String) -> ActionResult {
        let hint = subject.location?.focusHint?.trimmingCharacters(in: .whitespacesAndNewlines)
        let path = subject.location?.openURL?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let text = [hint, path].compactMap({ $0 }).first(where: { !$0.isEmpty }) else {
            return .failed(fallback)
        }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        return .succeeded("Copied location — \(fallback)")
    }

    /// Destructive kinds require explicit confirmation in UI.
    static func isDestructive(_ action: JobAction) -> Bool {
        if action.destructive { return true }
        switch action.kind.lowercased() {
        case "cancel", "reject":
            return true
        default:
            return action.confirmationRequired
        }
    }
}
