import AppKit

// MARK: - Nerve job state → menu-bar ribbon sizing

@MainActor
enum RibbonRenderer {
    static func preferredWidth(store: JobStore, settings: SettingsStore) -> CGFloat {
        let n = store.activeCount
        if n == 0 {
            let idle: CGFloat = (22 * settings.ribbonLengthScale).rounded()
            return settings.hideWhenIdle ? 0 : max(14, idle)
        }
        let factor = store.ribbonLengthFactor()
        let base: CGFloat = 28 + (100 - 28) * factor
        return min(120, max(16, base * CGFloat(settings.ribbonLengthScale))).rounded()
    }

    static func ambientRelevant(store: JobStore, settings: SettingsStore) -> Bool {
        let segments = store.ribbonSegments(mode: settings.panelGroupMode)
        guard !segments.isEmpty else { return false }
        switch settings.ribbonMotionStyle {
        case .transitionsOnly: return false
        case .breathe:
            return segments.contains { $0.status == .running || $0.status == .waiting || $0.status == .monitor }
        case .shimmer: return true
        case .statusPulse, .full:
            return segments.contains {
                switch $0.status {
                case .problem, .attention, .running, .waiting, .monitor: return true
                default: return false
                }
            }
        }
    }
}
