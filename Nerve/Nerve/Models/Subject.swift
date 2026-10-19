import Foundation

/// A unit of work on a machine (session, build, test, deploy, …).
/// Producer is who reported it; `alias` is which machine.
struct Job: Identifiable, Codable, Sendable, Hashable {
    var id: String
    /// `session`, `build`, `test`, `deploy`, `custom.*`, …
    var kind: String
    var name: String
    /// Machine alias (SSH Host / Settings alias).
    var alias: String

    var lifecycle: Lifecycle
    var current: Current?
    var attention: Attention
    var health: Health
    var outcome: Outcome?
    var progress: Progress
    var producer: ProducerInfo
    var context: ContextInfo?
    var location: LocationInfo?
    var capabilities: [String]
    var actions: [JobAction]

    var createdAt: Date
    var startedAt: Date?
    var endedAt: Date?
    var updatedAt: Date
    var version: UInt64

    /// Forward-compatible open fields from producers.
    var display: HubDisplay? = nil
    var extensions: [String: JSONValue]

    // MARK: Derived (not stored / not ingested)

    /// Single display status for every view (ribbon, panel, header counts).
    /// Derived only from structured facets — never free-text.
    var status: Status { Status(rawValue: display?.status ?? "inactive") ?? .inactive }
    var statusLabel: String { display?.statusLabel ?? "" }

    // MARK: Role / visibility (panel / ribbon)

    /// Explicit producer role from `extensions.role` (group | member | job | subagent | …).
    var extensionRole: String? { display?.role }

    /// What the human last asked this agent (`extensions.lastPrompt`).
    ///
    /// Reported once, by the producer's `UserPromptSubmit` hook run, and kept
    /// by the hub across the snapshots that follow (`STICKY_EXTENSIONS` in
    /// `crates/nerve-hub/src/state/store.rs`). It is the one thing a status row
    /// cannot say: `current` is what the agent is doing *now*.
    var lastPrompt: String? {
        guard case .string(let prompt) = extensions["lastPrompt"] else { return nil }
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    var isGroupJob: Bool { extensionRole == "group" }
    var isMemberJob: Bool { extensionRole == "member" }

    /// Group id for member rows (`extensions.groupId` or legacy `parentJobId`).
    var groupId: String? { display?.groupId }

    /// Whether this row should be kept in the in-memory job map.
    var isConversationJob: Bool { true }

    /// Whether this job paints a menu-bar ribbon segment.
    ///
    /// Members and rows with ``paintRibbon=false`` stay off the ribbon;
    /// Settings may further restrict roots.
    var paintsRibbon: Bool { display?.paintsRibbon ?? false }

    var isAskReason: Bool { display?.ask ?? false }
    var isAskElevated: Bool { display?.ask ?? false }

    static func make(
        id: String,
        kind: String = "session",
        name: String,
        alias: String,
        producer: ProducerInfo,
        lifecycle: Lifecycle = .active,
        now: Date = .now
    ) -> Job {
        Job(
            id: id,
            kind: kind,
            name: name,
            alias: alias,
            lifecycle: lifecycle,
            current: nil,
            attention: .none,
            health: .ok,
            outcome: nil,
            progress: .none,
            producer: producer,
            context: nil,
            location: nil,
            capabilities: [],
            actions: [],
            createdAt: now,
            startedAt: now,
            endedAt: nil,
            updatedAt: now,
            version: 1,
            extensions: [:]
        )
    }
}

// MARK: - JSONValue (open extensions)

enum JSONValue: Codable, Sendable, Hashable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let v = try? c.decode(Bool.self) {
            self = .bool(v)
        } else if let v = try? c.decode(Double.self) {
            self = .number(v)
        } else if let v = try? c.decode(String.self) {
            self = .string(v)
        } else if let v = try? c.decode([JSONValue].self) {
            self = .array(v)
        } else if let v = try? c.decode([String: JSONValue].self) {
            self = .object(v)
        } else {
            self = .null
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let v): try c.encode(v)
        case .number(let v): try c.encode(v)
        case .string(let v): try c.encode(v)
        case .array(let v): try c.encode(v)
        case .object(let v): try c.encode(v)
        }
    }
}

struct HubDisplay: Codable, Sendable, Hashable {
    var machineGroup: String?
    var machineLabel: String?
    var sourceOnline: Bool?
    var status: String
    var statusRank: Int
    var ask: Bool
    var busy: Bool
    var role: String
    var groupId: String?
    var paintsRibbon: Bool
    var section: String
    var statusLabel: String
    var activity: String
    var order: Int?
}
