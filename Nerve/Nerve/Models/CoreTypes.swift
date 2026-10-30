import Foundation

// MARK: - Lifecycle

/// Lifecycle stage only. Activity, attention, failure, etc. live in other facets.
enum Lifecycle: String, Codable, Sendable, Hashable, CaseIterable {
    case created = "created"
    case pending = "pending"
    case active = "active"
    case suspended = "suspended"
    case ended = "ended"
    case unknown = "unknown"

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = Lifecycle(rawValue: raw.lowercased()) ?? .unknown
    }
}

// MARK: - Attention

enum AttentionLevel: String, Codable, Sendable, Hashable, CaseIterable, Comparable {
    case none = "none"
    case informational = "informational"
    case suggested = "suggested"
    case required = "required"
    case urgent = "urgent"

    private var rank: Int {
        switch self {
        case .none: return 0
        case .informational: return 1
        case .suggested: return 2
        case .required: return 3
        case .urgent: return 4
        }
    }

    static func < (lhs: AttentionLevel, rhs: AttentionLevel) -> Bool {
        lhs.rank < rhs.rank
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = AttentionLevel(rawValue: raw.lowercased()) ?? .none
    }
}

struct Attention: Codable, Sendable, Hashable {
    var level: AttentionLevel
    var reason: String?
    var title: String?
    var summary: String?
    var deferrable: Bool?
    var deadline: Date?

    static let none = Attention(level: .none)

    init(
        level: AttentionLevel = .none,
        reason: String? = nil,
        title: String? = nil,
        summary: String? = nil,
        deferrable: Bool? = nil,
        deadline: Date? = nil
    ) {
        self.level = level
        self.reason = reason
        self.title = title
        self.summary = summary
        self.deferrable = deferrable
        self.deadline = deadline
    }
}

// MARK: - Health / Outcome

enum Health: String, Codable, Sendable, Hashable, CaseIterable {
    case ok = "ok"
    case degraded = "degraded"
    case unresponsive = "unresponsive"
    case unknown = "unknown"

    var severity: Int {
        switch self {
        case .ok: return 0
        case .unknown: return 1
        case .degraded: return 2
        case .unresponsive: return 3
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = Health(rawValue: raw.lowercased()) ?? .unknown
    }
}

enum Outcome: String, Codable, Sendable, Hashable, CaseIterable {
    case success = "success"
    case failure = "failure"
    case cancelled = "cancelled"
    case partial = "partial"
    case unknown = "unknown"

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = Outcome(rawValue: raw.lowercased()) ?? .unknown
    }
}

// MARK: - Current

/// What the Subject is doing now. Not an independent run object.
struct Current: Codable, Sendable, Hashable {
    /// Open type string, e.g. planning, testing, custom.foo
    var type: String
    var name: String?
    var summary: String?
    var detail: String?
    var startedAt: Date?

    init(
        type: String,
        name: String? = nil,
        summary: String? = nil,
        detail: String? = nil,
        startedAt: Date? = nil
    ) {
        self.type = type
        self.name = name
        self.summary = summary
        self.detail = detail
        self.startedAt = startedAt
    }
}

// MARK: - Progress

enum ProgressKind: String, Codable, Sendable, Hashable {
    case none
    case indeterminate
    case determinate
    case metrics
}

struct ProgressMetric: Codable, Sendable, Hashable {
    var label: String
    var current: Double?
    var total: Double?
    var unit: String?
}

struct Progress: Codable, Sendable, Hashable {
    var kind: ProgressKind
    /// Only when source provides a trustworthy ratio in 0...1. Never invent.
    var ratio: Double?
    var label: String?
    var metrics: [ProgressMetric]?

    static let none = Progress(kind: .none)

    init(
        kind: ProgressKind = .none,
        ratio: Double? = nil,
        label: String? = nil,
        metrics: [ProgressMetric]? = nil
    ) {
        self.kind = kind
        self.ratio = ratio
        self.label = label
        self.metrics = metrics
    }
}

// MARK: - Producer / Context / Location

/// Who reported the job (claude-code, codex, xcode, …). Not a machine.
struct ProducerInfo: Codable, Sendable, Hashable {
    var id: String
    var name: String?
    var kind: String?

    init(id: String, name: String? = nil, kind: String? = nil) {
        self.id = id
        self.name = name
        self.kind = kind
    }
}

struct ContextInfo: Codable, Sendable, Hashable {
    var project: String?
    var workspace: String?
    var labels: [String]?
}

struct LocationInfo: Codable, Sendable, Hashable {
    var openURL: String?
    var focusHint: String?
    var logPath: String?
}

// MARK: - Action

enum ActionState: String, Codable, Sendable, Hashable {
    case available
    case pending
    case succeeded
    case failed
    case expired
    case unsupported
}

struct JobAction: Codable, Sendable, Hashable, Identifiable {
    var id: String
    var title: String
    var kind: String
    var state: ActionState
    var destructive: Bool
    var confirmationRequired: Bool

    init(
        id: String,
        title: String,
        kind: String,
        state: ActionState = .available,
        destructive: Bool = false,
        confirmationRequired: Bool = false
    ) {
        self.id = id
        self.title = title
        self.kind = kind
        self.state = state
        self.destructive = destructive
        self.confirmationRequired = confirmationRequired
    }
}

// MARK: - Job status (model)

/// Derived display status of a job — one value for all views (ribbon, panel, counts).
/// Not an ingest facet; computed from lifecycle / health / outcome / attention / current.
/// `allCases` order = priority when grouping or painting segments.
enum Status: String, Sendable, Hashable, CaseIterable, Comparable {
    /// Red — failed or cannot continue.
    case problem
    /// Orange — needs human input, approval, or review.
    case attention
    /// Gray — automatic wait on the system; no human action needed.
    case waiting
    /// Blue — actively executing (main thread, or a background shell/subagent).
    case running
    /// Purple — watching a background stream; phase done, not executing.
    case monitor
    /// Green — completed turn or ended success.
    case success
    /// Gray — ready (no turn yet), paused, or unknown. Not “dead”.
    case inactive

    /// Statuses the ribbon, panel and Settings paint. Waiting is neutral gray.
    static let painted: [Status] = [.problem, .attention, .waiting, .running, .monitor, .success, .inactive]

    private var order: Int {
        switch self {
        case .problem: return 0
        case .attention: return 1
        case .waiting: return 2
        case .running: return 3
        case .monitor: return 4
        case .success: return 5
        case .inactive: return 6
        }
    }

    static func < (lhs: Status, rhs: Status) -> Bool {
        lhs.order < rhs.order
    }

    var title: String {
        switch self {
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .attention: return "Needs you"
        case .problem: return "Problem"
        case .monitor: return "Monitor"
        case .success: return "Completed"
        case .inactive: return "Ready / idle"
        }
    }

    /// problem / attention get a minimum segment weight when painting the ribbon.
    var isHighPriority: Bool {
        switch self {
        case .problem, .attention: return true
        default: return false
        }
    }
}

// MARK: - Panel row columns (view preference)

/// Configurable text columns in a status-panel row (dot + chevron are fixed chrome).
enum PanelColumn: String, Codable, CaseIterable, Identifiable, Sendable, Hashable {
    case name
    case summary
    case producer
    case machine
    case status
    case updated

    var id: String { rawValue }

    var title: String {
        switch self {
        case .name: return "Name"
        case .summary: return "Activity"
        case .producer: return "Producer"
        case .machine: return "Machine"
        case .status: return "Status"
        case .updated: return "Updated"
        }
    }

    var help: String {
        switch self {
        case .name: return "Job title (often the project name)"
        case .summary: return "Current activity or attention title"
        case .producer: return "Who reported it (Claude Code, Grok, …)"
        case .machine: return "Machine alias from the snapshot"
        case .status: return "Derived status label"
        case .updated: return "Time of last update"
        }
    }

    /// Retired columns. They stay in the enum so saved preferences still
    /// decode, but nothing offers them and nothing paints them — status is the
    /// row dot, and producer is not shown at all.
    var isRenderable: Bool {
        switch self {
        case .producer, .status: return false
        default: return true
        }
    }

    /// Flexible columns absorb leftover width so rows stay aligned.
    var isFlexible: Bool {
        switch self {
        case .summary: return true
        default: return false
        }
    }

    /// Ideal width. The name stops growing here; activity has no ideal.
    var width: CGFloat {
        switch self {
        case .name: return 108
        case .summary: return 0 // flexible
        case .producer: return 72
        case .machine: return 72
        case .status: return 72
        case .updated: return 72
        }
    }

    /// Narrowest this column may be before the row is tighter than the panel.
    /// A locked 108pt name on the 340pt panel left activity a few characters wide.
    var minWidth: CGFloat {
        switch self {
        case .name: return 72
        case .summary: return 96
        case .updated: return 56
        default: return width
        }
    }

    var maxWidth: CGFloat {
        switch self {
        case .name: return 108
        case .summary: return .infinity
        case .updated: return width
        default: return isFlexible ? .infinity : width
        }
    }

    /// Activity keeps its width. The name and the time give way, and the
    /// full string stays on hover.
    var layoutPriority: Double { isFlexible ? 1 : 0 }

    static let defaultColumns: [PanelColumn] = [.name, .summary, .updated]

    /// Columns users can toggle — every column that still paints something.
    static var configurableColumns: [PanelColumn] { allCases.filter(\.isRenderable) }
}

// MARK: - Panel member visibility (view preference)

/// How group member (leaf) jobs appear in the status panel.
/// Style only — producers still POST members; this filters display.
enum PanelMemberVisibility: String, Codable, CaseIterable, Identifiable, Sendable, Hashable {
    /// Never list member rows (group summary only).
    case never
    /// Only members with elevated attention / problem status (default).
    case attention
    /// List every stored member under its group when expanded (or flat).
    case all

    var id: String { rawValue }

    var title: String {
        switch self {
        case .never: return "Groups only"
        case .attention: return "Problems & attention"
        case .all: return "All members"
        }
    }

    var help: String {
        switch self {
        case .never:
            return "Show group rows only; hide individual members."
        case .attention:
            return "Show groups, plus members that need attention or failed."
        case .all:
            return "Show every member under its group (can get long)."
        }
    }
}

// MARK: - Ribbon ambient motion (view preference)

/// Continuous ribbon motion while work is open. Transitions (length / segment
/// cross-fades) are controlled separately by `animationsEnabled`.
enum RibbonMotionStyle: String, Codable, CaseIterable, Identifiable, Sendable, Hashable {
    /// Only ease when segments or length change (default, quiet).
    case transitionsOnly
    /// Soft brightness breathe on Running / Waiting.
    case breathe
    /// Soft highlight sweeps across the band while work is open.
    case shimmer
    /// Status-aware: Running breathe, Attention blink, Problem urgent pulse.
    case statusPulse
    /// Shimmer plus status-aware pulses.
    case full

    var id: String { rawValue }

    var title: String {
        switch self {
        case .transitionsOnly: return "Transitions only"
        case .breathe: return "Breathe"
        case .shimmer: return "Shimmer"
        case .statusPulse: return "Status pulse"
        case .full: return "Full"
        }
    }

    var help: String {
        switch self {
        case .transitionsOnly:
            return "Ease on change only. Static while settled."
        case .breathe:
            return "Running segments gently brighten and dim."
        case .shimmer:
            return "A soft highlight sweeps the ribbon while work is open."
        case .statusPulse:
            return "Running breathes, attention soft-blinks, problems pulse."
        case .full:
            return "Shimmer plus status pulses on running, attention, and problem."
        }
    }

    /// True when the ribbon should redraw on a clock while segments allow it.
    var usesAmbientMotion: Bool {
        self != .transitionsOnly
    }
}

// MARK: - Panel grouping (view preference)

/// How the status panel buckets jobs into sections.
/// Grouping only — within a section, jobs still sort by attention → health → …
enum PanelGroupMode: String, Codable, CaseIterable, Identifiable, Sendable, Hashable {
    /// Bucket by reported machine alias (default).
    case machine
    /// Attention / Active / Recent (from `Status`).
    case priority
    /// One section per `Status`.
    case status

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "source": self = .machine
        default: self = PanelGroupMode(rawValue: raw) ?? .machine
        }
    }

    var id: String { rawValue }

    var title: String {
        switch self {
        case .machine: return "Machine"
        case .priority: return "Priority"
        case .status: return "Status"
        }
    }

    var shortTitle: String {
        switch self {
        case .machine: return "Machine"
        case .priority: return "Prio"
        case .status: return "Status"
        }
    }
}

/// One collapsible section in the status panel list.
struct StatusSection: Identifiable, Hashable {
    var id: String
    var title: String
    var jobs: [Job]

    init(id: String, title: String, jobs: [Job]) {
        self.id = id
        self.title = title
        self.jobs = jobs
    }
}
