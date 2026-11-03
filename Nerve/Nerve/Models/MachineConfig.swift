import Foundation

/// A remote tunnel row in Settings — sourced from the user's `~/.ssh/config` Hosts.
/// Job display uses whatever `alias` the producer reports; this list is tunnels only.
struct MachineConfig: Identifiable, Codable, Hashable, Sendable {
    var id: UUID
    /// SSH `Host` token (`ssh <alias>`).
    var alias: String
    /// `HostName` from the user's config (display).
    var hostName: String
    var user: String
    var sshPort: UInt16
    /// Absolute path when set in the user's config (display).
    var identityFile: String?
    var enabled: Bool
    var autoConnect: Bool
    /// Port forwarded on the remote loopback to this Mac's Nerve ingest.
    var remoteIngestPort: UInt16

    init(
        id: UUID = UUID(),
        alias: String,
        hostName: String,
        user: String = NSUserName(),
        sshPort: UInt16 = 22,
        identityFile: String? = nil,
        enabled: Bool = true,
        autoConnect: Bool = true,
        remoteIngestPort: UInt16 = 17890
    ) {
        self.id = id
        self.alias = Self.sanitizeAlias(alias)
        self.hostName = hostName.trimmingCharacters(in: .whitespacesAndNewlines)
        self.user = user.trimmingCharacters(in: .whitespacesAndNewlines)
        self.sshPort = sshPort
        self.identityFile = identityFile
        self.enabled = enabled
        self.autoConnect = autoConnect
        self.remoteIngestPort = remoteIngestPort
    }

    enum CodingKeys: String, CodingKey {
        case id, alias, hostName, user, sshPort, identityFile
        case enabled, autoConnect, remoteIngestPort
        // Legacy key ignored on decode.
        case usesExistingSSHHost
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        alias = try c.decode(String.self, forKey: .alias)
        hostName = try c.decode(String.self, forKey: .hostName)
        user = try c.decode(String.self, forKey: .user)
        sshPort = try c.decode(UInt16.self, forKey: .sshPort)
        identityFile = try c.decodeIfPresent(String.self, forKey: .identityFile)
        enabled = try c.decode(Bool.self, forKey: .enabled)
        autoConnect = try c.decode(Bool.self, forKey: .autoConnect)
        remoteIngestPort = try c.decode(UInt16.self, forKey: .remoteIngestPort)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(alias, forKey: .alias)
        try c.encode(hostName, forKey: .hostName)
        try c.encode(user, forKey: .user)
        try c.encode(sshPort, forKey: .sshPort)
        try c.encodeIfPresent(identityFile, forKey: .identityFile)
        try c.encode(enabled, forKey: .enabled)
        try c.encode(autoConnect, forKey: .autoConnect)
        try c.encode(remoteIngestPort, forKey: .remoteIngestPort)
    }

    /// SSH Host tokens: letters, digits, `.`, `-`, `_`. Empty input stays empty.
    static func sanitizeAlias(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: ".-_"))
        return String(trimmed.unicodeScalars.filter { allowed.contains($0) })
    }
}

/// Runtime connection status for a tunnel (not persisted).
enum MachineLinkState: String, Sendable, Hashable {
    case idle
    case connecting
    case connected
    case error
}

/// This Mac — labels for Settings / demo data only. Ingest jobs keep their own alias.
enum LocalMachine {
    /// Stable Bonjour name when available; otherwise process hostname short form.
    /// Cached once: `scutil` is a subprocess and must not run on every SwiftUI body eval.
    static let alias: String = resolveAlias()

    static var kind: String {
        #if os(macOS)
        return "darwin"
        #else
        return "unknown"
        #endif
    }

    private static func resolveAlias() -> String {
        if let local = bonjourLocalHostName(), !local.isEmpty {
            return MachineConfig.sanitizeAlias(local)
        }
        let host = ProcessInfo.processInfo.hostName
        if let short = host.split(separator: ".").first.map(String.init), !short.isEmpty {
            return MachineConfig.sanitizeAlias(short)
        }
        return "local"
    }

    private static func bonjourLocalHostName() -> String? {
        #if os(macOS)
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/sbin/scutil")
        task.arguments = ["--get", "LocalHostName"]
        let pipe = Pipe()
        task.standardOutput = pipe
        task.standardError = FileHandle.nullDevice
        do {
            try task.run()
            task.waitUntilExit()
            guard task.terminationStatus == 0 else { return nil }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let s = String(data: data, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return (s?.isEmpty == false) ? s : nil
        } catch {
            return nil
        }
        #else
        return nil
        #endif
    }
}

/// Which machine a job runs on, and how this Mac can reach it.
///
/// The hub says. A job it reached through `ssh <Host>` carries that Host as
/// `extensions.sshHost` — a stanza of the same `~/.ssh/config` the Machines
/// list is read from, so it is also how to get there. Every other job is this
/// Mac's: a hook only ever posts to the hub on its own machine. Asking first
/// is what keeps a remote workspace path from being opened locally.
enum RemoteMachine {
    /// The ssh `Host` that reached this job's machine, or `nil` for this Mac.
    static func sshHost(of job: Job) -> String? {
        guard case .string(let raw) = job.extensions["sshHost"] else { return nil }
        let host = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return host.isEmpty ? nil : host
    }
}
