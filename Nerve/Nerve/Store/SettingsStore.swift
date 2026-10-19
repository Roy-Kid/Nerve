import Foundation
import CoreGraphics
import AppKit
import SwiftUI

// MARK: - Status colors

/// sRGB triple 0…1, persisted with preferences (not subject data).
struct RGBColor: Codable, Equatable, Sendable, Hashable {
    var r: Double
    var g: Double
    var b: Double

    var color: Color { Color(red: r, green: g, blue: b) }

    var nsColor: NSColor {
        NSColor(srgbRed: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1)
    }

    init(r: Double, g: Double, b: Double) {
        self.r = min(1, max(0, r))
        self.g = min(1, max(0, g))
        self.b = min(1, max(0, b))
    }

    init(_ ns: NSColor) {
        let c = ns.usingColorSpace(.sRGB) ?? ns
        var rr: CGFloat = 0, gg: CGFloat = 0, bb: CGFloat = 0, aa: CGFloat = 0
        c.getRed(&rr, green: &gg, blue: &bb, alpha: &aa)
        self.init(r: Double(rr), g: Double(gg), b: Double(bb))
    }

    init(_ color: Color) {
        self.init(NSColor(color))
    }
}

/// User-customizable mapping from job `Status` → color. Defaults follow macOS system colors.
struct StatusColorMap: Codable, Equatable, Sendable {
    var problem: RGBColor
    var attention: RGBColor
    var waiting: RGBColor
    var running: RGBColor
    var monitor: RGBColor
    var success: RGBColor
    var inactive: RGBColor

    /// Rainbow violet `#BF5AF2` — used when a saved palette predates `monitor`.
    private static let monitorDefault = RGBColor(r: 0.749, g: 0.353, b: 0.949)

    static let `default` = StatusColorMap(
        problem: RGBColor(r: 1.000, g: 0.231, b: 0.188),   // #FF3B30 rainbow red
        attention: RGBColor(r: 1.000, g: 0.624, b: 0.039), // #FF9F0A rainbow orange
        waiting: RGBColor(r: 0.557, g: 0.557, b: 0.576),   // automatic wait, no human action
        running: RGBColor(r: 0.039, g: 0.518, b: 1.000),   // #0A84FF rainbow blue
        monitor: monitorDefault,                           // #BF5AF2 rainbow violet
        success: RGBColor(r: 0.188, g: 0.820, b: 0.345),   // #30D158 rainbow green
        inactive: RGBColor(r: 0.557, g: 0.557, b: 0.576)    // #8E8E93 gray, not rainbow
    )

    init(
        problem: RGBColor,
        attention: RGBColor,
        waiting: RGBColor,
        running: RGBColor,
        monitor: RGBColor = monitorDefault,
        success: RGBColor,
        inactive: RGBColor
    ) {
        self.problem = problem
        self.attention = attention
        self.waiting = waiting
        self.running = running
        self.monitor = monitor
        self.success = success
        self.inactive = inactive
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        problem = try c.decode(RGBColor.self, forKey: .problem)
        attention = try c.decode(RGBColor.self, forKey: .attention)
        waiting = try c.decode(RGBColor.self, forKey: .waiting)
        running = try c.decode(RGBColor.self, forKey: .running)
        monitor = try c.decodeIfPresent(RGBColor.self, forKey: .monitor) ?? Self.monitorDefault
        success = try c.decode(RGBColor.self, forKey: .success)
        inactive = try c.decode(RGBColor.self, forKey: .inactive)
    }

    /// Every palette that used to be `default`, newest first. An install still
    /// sitting on one of these never customized its colors, so it is safe to
    /// move forward — add a row here when `default` changes, nothing else.
    static let retiredDefaults: [StatusColorMap] = [
        StatusColorMap(problem: .init(r: 1, g: 0.231, b: 0.188), attention: .init(r: 1, g: 0.624, b: 0.039), waiting: .init(r: 1, g: 0.624, b: 0.039), running: .init(r: 0.039, g: 0.518, b: 1), success: .init(r: 0.188, g: 0.820, b: 0.345), inactive: .init(r: 0.557, g: 0.557, b: 0.576)),
        // Six distinct hues, waiting was purple.
        StatusColorMap(
            problem: RGBColor(r: 1.000, g: 0.271, b: 0.227),
            attention: RGBColor(r: 1.000, g: 0.624, b: 0.039),
            waiting: RGBColor(r: 0.749, g: 0.353, b: 0.949),
            running: RGBColor(r: 0.039, g: 0.518, b: 1.000),
            success: RGBColor(r: 0.188, g: 0.820, b: 0.345),
            inactive: RGBColor(r: 0.557, g: 0.557, b: 0.576)
        ),
        // Before the vivid alignment pass.
        StatusColorMap(
            problem: RGBColor(r: 1.000, g: 0.231, b: 0.188),
            attention: RGBColor(r: 1.000, g: 0.584, b: 0.000),
            waiting: RGBColor(r: 0.686, g: 0.321, b: 0.871),
            running: RGBColor(r: 0.000, g: 0.478, b: 1.000),
            success: RGBColor(r: 0.204, g: 0.780, b: 0.349),
            inactive: RGBColor(r: 0.557, g: 0.557, b: 0.576)
        ),
        // First release.
        StatusColorMap(
            problem: RGBColor(r: 0.95, g: 0.14, b: 0.18),
            attention: RGBColor(r: 1.00, g: 0.52, b: 0.10),
            waiting: RGBColor(r: 0.55, g: 0.28, b: 0.92),
            running: RGBColor(r: 0.14, g: 0.52, b: 0.98),
            success: RGBColor(r: 0.08, g: 0.80, b: 0.34),
            inactive: RGBColor(r: 0.48, g: 0.52, b: 0.58)
        ),
    ]

    func color(for status: Status) -> RGBColor {
        switch status {
        case .problem: return problem
        case .attention: return attention
        case .waiting: return waiting
        case .running: return running
        case .monitor: return monitor
        case .success: return success
        case .inactive: return inactive
        }
    }

    mutating func set(_ color: RGBColor, for status: Status) {
        switch status {
        case .problem: problem = color
        case .attention: attention = color
        case .waiting: waiting = color
        case .running: running = color
        case .monitor: monitor = color
        case .success: success = color
        case .inactive: inactive = color
        }
    }

    var isDefault: Bool { self == .default }

    /// Bump installs that never customized colors to the current vivid palette.
    static func migrated(from saved: StatusColorMap) -> StatusColorMap {
        retiredDefaults.contains(saved) ? .default : saved
    }
}

// MARK: - Settings

@MainActor
@Observable
final class SettingsStore {
    /// Fired after any preference that affects the menu-bar ribbon image/length.
    /// Set by AppModel so grouping / size / color changes redraw immediately
    /// without depending on a particular SwiftUI view still being alive.
    var ribbonAppearanceSink: (() -> Void)?

    var hideWhenIdle: Bool {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }
    /// Legacy sticky flag (no longer gates animation by itself).
    var reduceMotion: Bool {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }
    var animationsEnabled: Bool {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }
    /// When true (default), honor macOS Accessibility Reduce Motion.
    /// Turn off to keep ribbon motion while system Reduce Motion is enabled.
    var respectSystemReduceMotion: Bool {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }
    /// Continuous menu-bar motion while work is open (see `RibbonMotionStyle`).
    /// Requires `effectiveAnimationsEnabled`; otherwise the ribbon stays static.
    var ribbonMotionStyle: RibbonMotionStyle {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }
    /// Per-status ribbon / panel colors.
    var statusColors: StatusColorMap {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }

    var notificationsPaused: Bool { didSet { persist() } }
    var notifyRequired: Bool { didSet { persist() } }
    var notifyUrgent: Bool { didSet { persist() } }
    var notifyFailure: Bool { didSet { persist() } }
    var notifyUnresponsive: Bool { didSet { persist() } }
    var notifyLongSuccess: Bool { didSet { persist() } }
    var notificationSoundEnabled: Bool { didSet { persist() } }
    var longTaskSuccessThresholdSeconds: TimeInterval { didSet { persist() } }
    var mutedSourceIds: Set<String> { didSet { persist() } }
    var mutedProjectIds: Set<String> { didSet { persist() } }

    /// Do-not-disturb schedule (local calendar minutes from midnight).
    var dndEnabled: Bool { didSet { persist() } }
    var dndStartMinutes: Int { didSet { persist() } }
    var dndEndMinutes: Int { didSet { persist() } }

    var hasCompletedFirstRun: Bool { didSet { persist() } }
    var hasSeenCoachMarks: Bool { didSet { persist() } }
    /// Status panel section grouping (priority / status / source).
    /// Also drives how the menu-bar ribbon is segmented and ordered.
    var panelGroupMode: PanelGroupMode {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }

    /// Which group-member leaf jobs appear in the status panel (style preference).
    var panelMemberVisibility: PanelMemberVisibility {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }

    /// When true (default), only root jobs (`paintsRibbon`) color the menu-bar ribbon.
    /// Turn off to also paint elevated-attention members (style preference).
    var ribbonRootsOnly: Bool {
        didSet {
            persist()
            notifyRibbonAppearance()
        }
    }

    /// Ordered text columns shown in each status-panel row.
    /// Dot + chevron stay fixed; this only configures the middle fields.
    var panelColumns: [PanelColumn] {
        didSet {
            let normalized = Self.normalizePanelColumns(panelColumns)
            if panelColumns != normalized {
                panelColumns = normalized
                return
            }
            persist()
        }
    }

    /// Horizontal length scale for the menu-bar ribbon (0.5…4.0, default 1.0).
    var ribbonLengthScale: Double {
        didSet {
            let clamped = Self.clampRibbonLengthScale(ribbonLengthScale)
            if ribbonLengthScale != clamped {
                ribbonLengthScale = clamped
                return
            }
            persist()
            notifyRibbonAppearance()
        }
    }
    /// Vertical thickness of the ribbon bar in points (3…12, default 6).
    var ribbonThickness: Double {
        didSet {
            let clamped = Self.clampRibbonThickness(ribbonThickness)
            if ribbonThickness != clamped {
                ribbonThickness = clamped
                return
            }
            persist()
            notifyRibbonAppearance()
        }
    }

    /// Status panel width in points (user-resizable; remembered across opens).
    var panelWidth: Double {
        didSet {
            let clamped = Self.clampPanelWidth(panelWidth)
            if panelWidth != clamped {
                panelWidth = clamped
                return
            }
            persist()
        }
    }

    /// Status panel height in points (user-resizable; remembered across opens).
    var panelHeight: Double {
        didSet {
            let clamped = Self.clampPanelHeight(panelHeight)
            if panelHeight != clamped {
                panelHeight = clamped
                return
            }
            persist()
        }
    }

    /// Remote machines configured in Settings (SSH Host alias + tunnel). Local Mac is implicit.
    var machines: [MachineConfig]

    private let defaults = UserDefaults.standard
    private let key = "nerve.settings.v5"
    private var isLoading = true

    private func notifyRibbonAppearance() {
        guard !isLoading else { return }
        ribbonAppearanceSink?()
    }

    static let defaultRibbonLengthScale: Double = 1.0
    static let defaultRibbonThickness: Double = 6.0
    static let ribbonLengthScaleRange: ClosedRange<Double> = 0.5...4.0
    static let ribbonThicknessRange: ClosedRange<Double> = 3...12

    static let defaultPanelWidth: Double = 340
    static let defaultPanelHeight: Double = 360
    static let panelWidthRange: ClosedRange<Double> = 280...560
    static let panelHeightRange: ClosedRange<Double> = 180...720

    static func clampRibbonLengthScale(_ v: Double) -> Double {
        min(ribbonLengthScaleRange.upperBound, max(ribbonLengthScaleRange.lowerBound, v))
    }

    static func clampRibbonThickness(_ v: Double) -> Double {
        min(ribbonThicknessRange.upperBound, max(ribbonThicknessRange.lowerBound, v))
    }

    static func clampPanelWidth(_ v: Double) -> Double {
        min(panelWidthRange.upperBound, max(panelWidthRange.lowerBound, v))
    }

    static func clampPanelHeight(_ v: Double) -> Double {
        min(panelHeightRange.upperBound, max(panelHeightRange.lowerBound, v))
    }

    /// Keep enum order, drop retired columns, guarantee at least `.name`.
    static func normalizePanelColumns(_ cols: [PanelColumn]) -> [PanelColumn] {
        var seen = Set<PanelColumn>()
        var ordered: [PanelColumn] = []
        for col in PanelColumn.allCases where col.isRenderable && cols.contains(col) {
            if seen.insert(col).inserted {
                ordered.append(col)
            }
        }
        if ordered.isEmpty { return PanelColumn.defaultColumns }
        return ordered
    }

    func isPanelColumnEnabled(_ col: PanelColumn) -> Bool {
        panelColumns.contains(col)
    }

    func setPanelColumn(_ col: PanelColumn, enabled: Bool) {
        var next = panelColumns
        if enabled {
            if !next.contains(col) { next.append(col) }
        } else {
            next.removeAll { $0 == col }
        }
        panelColumns = Self.normalizePanelColumns(next)
    }

    init() {
        hideWhenIdle = false
        reduceMotion = false
        animationsEnabled = true
        respectSystemReduceMotion = true
        ribbonMotionStyle = .transitionsOnly
        statusColors = .default
        notificationsPaused = false
        notifyRequired = true
        notifyUrgent = true
        notifyFailure = true
        notifyUnresponsive = true
        notifyLongSuccess = true
        notificationSoundEnabled = true
        longTaskSuccessThresholdSeconds = 300
        mutedSourceIds = []
        mutedProjectIds = []
        dndEnabled = false
        dndStartMinutes = 22 * 60
        dndEndMinutes = 8 * 60
        hasCompletedFirstRun = false
        hasSeenCoachMarks = false
        panelGroupMode = .priority
        panelMemberVisibility = .attention
        ribbonRootsOnly = true
        panelColumns = PanelColumn.defaultColumns
        ribbonLengthScale = Self.defaultRibbonLengthScale
        ribbonThickness = Self.defaultRibbonThickness
        panelWidth = Self.defaultPanelWidth
        panelHeight = Self.defaultPanelHeight
        machines = []

        if let data = UserDefaults.standard.data(forKey: key),
           let p = try? JSONDecoder().decode(Persisted.self, from: data) {
            apply(p)
        } else if let data = UserDefaults.standard.data(forKey: "nerve.settings.v4"),
                  let p = try? JSONDecoder().decode(Persisted.self, from: data) {
            apply(p)
        } else if let data = UserDefaults.standard.data(forKey: "nerve.settings.v3"),
                  let p = try? JSONDecoder().decode(PersistedV3.self, from: data) {
            applyV3(p)
        }

        // Do not sticky-persist system Reduce Motion into prefs — that once forced
        // animations off forever with no UI to reverse it. Live check only below.
        isLoading = false
        persist()
    }

    private func apply(_ p: Persisted) {
        hideWhenIdle = p.hideWhenIdle
        reduceMotion = p.reduceMotion
        animationsEnabled = p.animationsEnabled
        // Default true for older prefs; only false when user explicitly opted out.
        respectSystemReduceMotion = p.respectSystemReduceMotion ?? true
        ribbonMotionStyle = p.ribbonMotionStyle ?? .transitionsOnly
        let savedColors = p.statusColors ?? .default
        statusColors = StatusColorMap.migrated(from: savedColors)
        notificationsPaused = p.notificationsPaused
        notifyRequired = p.notifyRequired
        notifyUrgent = p.notifyUrgent
        notifyFailure = p.notifyFailure
        notifyUnresponsive = p.notifyUnresponsive
        notifyLongSuccess = p.notifyLongSuccess
        notificationSoundEnabled = p.notificationSoundEnabled
        longTaskSuccessThresholdSeconds = p.longTaskSuccessThresholdSeconds
        mutedSourceIds = Set(p.mutedSourceIds)
        mutedProjectIds = Set(p.mutedProjectIds)
        dndEnabled = p.dndEnabled
        dndStartMinutes = p.dndStartMinutes
        dndEndMinutes = p.dndEndMinutes
        hasCompletedFirstRun = p.hasCompletedFirstRun
        hasSeenCoachMarks = p.hasSeenCoachMarks
        if let mode = p.panelGroupMode {
            panelGroupMode = mode
        } else {
            panelGroupMode = .priority
        }
        panelMemberVisibility = p.panelMemberVisibility ?? .attention
        ribbonRootsOnly = p.ribbonRootsOnly ?? true
        panelColumns = Self.normalizePanelColumns(p.panelColumns ?? PanelColumn.defaultColumns)
        ribbonLengthScale = Self.clampRibbonLengthScale(p.ribbonLengthScale ?? Self.defaultRibbonLengthScale)
        ribbonThickness = Self.clampRibbonThickness(p.ribbonThickness ?? Self.defaultRibbonThickness)
        panelWidth = Self.clampPanelWidth(p.panelWidth ?? Self.defaultPanelWidth)
        panelHeight = Self.clampPanelHeight(p.panelHeight ?? Self.defaultPanelHeight)
        machines = p.machines ?? []
    }

    private func applyV3(_ p: PersistedV3) {
        hideWhenIdle = p.hideWhenIdle
        reduceMotion = p.reduceMotion
        animationsEnabled = p.animationsEnabled
        respectSystemReduceMotion = true
        ribbonMotionStyle = .transitionsOnly
        notificationsPaused = p.notificationsPaused
        notifyRequired = p.notifyRequired
        notifyUrgent = p.notifyUrgent
        notifyFailure = p.notifyFailure
        notifyUnresponsive = p.notifyUnresponsive
        notifyLongSuccess = p.notifyLongSuccess
        notificationSoundEnabled = p.notificationSoundEnabled
        longTaskSuccessThresholdSeconds = p.longTaskSuccessThresholdSeconds
        mutedSourceIds = Set(p.mutedSourceIds)
        mutedProjectIds = Set(p.mutedProjectIds)
        dndEnabled = p.dndEnabled
        dndStartMinutes = p.dndStartMinutes
        dndEndMinutes = p.dndEndMinutes
        hasCompletedFirstRun = p.hasCompletedFirstRun
        hasSeenCoachMarks = p.hasSeenCoachMarks
        panelGroupMode = p.panelGroupMode ?? .priority
        panelMemberVisibility = .attention
        ribbonRootsOnly = true
        panelColumns = PanelColumn.defaultColumns
        ribbonLengthScale = Self.defaultRibbonLengthScale
        ribbonThickness = Self.defaultRibbonThickness
        panelWidth = Self.defaultPanelWidth
        panelHeight = Self.defaultPanelHeight
        // Privacy/storage flags intentionally dropped — subjects are memory-only.
    }

    func resetStatusColors() {
        statusColors = .default
    }

    /// True when notifications are allowed right now.
    func shouldDeliverNotifications(at date: Date = Date()) -> Bool {
        if notificationsPaused { return false }
        if !dndEnabled { return true }
        return !isInDND(at: date)
    }

    func isInDND(at date: Date = Date()) -> Bool {
        guard dndEnabled else { return false }
        let cal = Calendar.current
        let comps = cal.dateComponents([.hour, .minute], from: date)
        let mins = (comps.hour ?? 0) * 60 + (comps.minute ?? 0)
        let start = dndStartMinutes
        let end = dndEndMinutes
        if start == end { return true }
        if start < end {
            return mins >= start && mins < end
        }
        return mins >= start || mins < end
    }

    var dndStartLabel: String { Self.formatMinutes(dndStartMinutes) }
    var dndEndLabel: String { Self.formatMinutes(dndEndMinutes) }

    static func formatMinutes(_ m: Int) -> String {
        let clamped = ((m % (24 * 60)) + (24 * 60)) % (24 * 60)
        return String(format: "%02d:%02d", clamped / 60, clamped % 60)
    }

    func cycleDNDStart() {
        let options = [21 * 60, 22 * 60, 23 * 60, 0]
        if let idx = options.firstIndex(of: dndStartMinutes) {
            dndStartMinutes = options[(idx + 1) % options.count]
        } else {
            dndStartMinutes = 22 * 60
        }
    }

    func cycleDNDEnd() {
        let options = [6 * 60, 7 * 60, 8 * 60, 9 * 60]
        if let idx = options.firstIndex(of: dndEndMinutes) {
            dndEndMinutes = options[(idx + 1) % options.count]
        } else {
            dndEndMinutes = 8 * 60
        }
    }

    func toggleMute(producerId: String) {
        if mutedSourceIds.contains(producerId) { mutedSourceIds.remove(producerId) }
        else { mutedSourceIds.insert(producerId) }
    }

    func toggleMuteProject(_ project: String) {
        if mutedProjectIds.contains(project) { mutedProjectIds.remove(project) }
        else { mutedProjectIds.insert(project) }
    }

    func upsertMachine(_ machine: MachineConfig) {
        if let idx = machines.firstIndex(where: { $0.id == machine.id }) {
            machines[idx] = machine
        } else {
            machines.append(machine)
        }
    }

    func removeMachine(id: UUID) {
        machines.removeAll { $0.id == id }
    }

    func machine(id: UUID) -> MachineConfig? {
        machines.first { $0.id == id }
    }

    /// Replace the tunnel list from concrete Hosts in `~/.ssh/config`.
    /// Keeps enabled / autoConnect / remoteIngestPort / id when the alias is unchanged.
    /// macOS Accessibility → Display → Reduce motion (live, not sticky prefs).
    var systemReduceMotionActive: Bool {
        NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
    }

    /// Transitions + ambient ribbon. User toggle, plus optional honor of system Reduce Motion.
    var effectiveAnimationsEnabled: Bool {
        guard animationsEnabled else { return false }
        if respectSystemReduceMotion && systemReduceMotionActive { return false }
        return true
    }

    private func persist() {
        guard !isLoading else { return }
        let value = Persisted(
            hideWhenIdle: hideWhenIdle,
            reduceMotion: reduceMotion,
            animationsEnabled: animationsEnabled,
            respectSystemReduceMotion: respectSystemReduceMotion,
            ribbonMotionStyle: ribbonMotionStyle,
            statusColors: statusColors,
            notificationsPaused: notificationsPaused,
            notifyRequired: notifyRequired,
            notifyUrgent: notifyUrgent,
            notifyFailure: notifyFailure,
            notifyUnresponsive: notifyUnresponsive,
            notifyLongSuccess: notifyLongSuccess,
            notificationSoundEnabled: notificationSoundEnabled,
            longTaskSuccessThresholdSeconds: longTaskSuccessThresholdSeconds,
            mutedSourceIds: Array(mutedSourceIds).sorted(),
            mutedProjectIds: Array(mutedProjectIds).sorted(),
            dndEnabled: dndEnabled,
            dndStartMinutes: dndStartMinutes,
            dndEndMinutes: dndEndMinutes,
            hasCompletedFirstRun: hasCompletedFirstRun,
            hasSeenCoachMarks: hasSeenCoachMarks,
            panelGroupMode: panelGroupMode,
            panelMemberVisibility: panelMemberVisibility,
            ribbonRootsOnly: ribbonRootsOnly,
            panelColumns: panelColumns,
            ribbonLengthScale: ribbonLengthScale,
            ribbonThickness: ribbonThickness,
            panelWidth: panelWidth,
            panelHeight: panelHeight,
            machines: nil
        )
        if let data = try? JSONEncoder().encode(value) {
            defaults.set(data, forKey: key)
        }
    }

    /// Prefs written to `UserDefaults`.
    ///
    /// Older payloads carry a retired ingest-port key from when the endpoint
    /// was a user knob. That field is gone (the port is fixed — see
    /// `NerveEndpoint`), and because the synthesized `CodingKeys` names only
    /// the properties below, the synthesized `Decodable` never looks for the
    /// stale key and old prefs still load. Do **not** hand-write a
    /// `CodingKeys` case for it: unknown keys are ignored precisely because
    /// they are absent from the enum.
    private struct Persisted: Codable {
        var hideWhenIdle: Bool
        var reduceMotion: Bool
        var animationsEnabled: Bool
        var respectSystemReduceMotion: Bool?
        var ribbonMotionStyle: RibbonMotionStyle?
        var statusColors: StatusColorMap?
        var notificationsPaused: Bool
        var notifyRequired: Bool
        var notifyUrgent: Bool
        var notifyFailure: Bool
        var notifyUnresponsive: Bool
        var notifyLongSuccess: Bool
        var notificationSoundEnabled: Bool
        var longTaskSuccessThresholdSeconds: TimeInterval
        var mutedSourceIds: [String]
        var mutedProjectIds: [String]
        var dndEnabled: Bool
        var dndStartMinutes: Int
        var dndEndMinutes: Int
        var hasCompletedFirstRun: Bool
        var hasSeenCoachMarks: Bool
        var panelGroupMode: PanelGroupMode?
        var panelMemberVisibility: PanelMemberVisibility?
        var ribbonRootsOnly: Bool?
        var panelColumns: [PanelColumn]?
        var ribbonLengthScale: Double?
        var ribbonThickness: Double?
        var panelWidth: Double?
        var panelHeight: Double?
        var machines: [MachineConfig]?
    }

    /// Subset of v3 prefs we still care about (storage/privacy fields and the
    /// retired ingest-port knob drop out the same way ``Persisted`` drops them).
    private struct PersistedV3: Codable {
        var hideWhenIdle: Bool
        var reduceMotion: Bool
        var animationsEnabled: Bool
        var notificationsPaused: Bool
        var notifyRequired: Bool
        var notifyUrgent: Bool
        var notifyFailure: Bool
        var notifyUnresponsive: Bool
        var notifyLongSuccess: Bool
        var notificationSoundEnabled: Bool
        var longTaskSuccessThresholdSeconds: TimeInterval
        var mutedSourceIds: [String]
        var mutedProjectIds: [String]
        var dndEnabled: Bool
        var dndStartMinutes: Int
        var dndEndMinutes: Int
        var hasCompletedFirstRun: Bool
        var hasSeenCoachMarks: Bool
        var panelGroupMode: PanelGroupMode?
    }
}
