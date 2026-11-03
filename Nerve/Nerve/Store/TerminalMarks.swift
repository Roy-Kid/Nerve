import Foundation

/// One terminal's status, for a host that draws a dot beside that terminal.
/// The hub chose it (`tabIndicators`); this only gives it the ribbon's colour.
public struct TerminalMark: Sendable, Equatable {
    /// The machine, as the hub names it: the `~/.ssh/config` Host that
    /// reached a remote one, and this Mac's Bonjour name (`<name>.local`).
    public let host: String
    /// `/dev/ttysN`, `/dev/pts/N`: the agent's controlling tty, which in
    /// tmux is the pane's.
    public let terminal: String
    /// `0xRRGGBB`, from the ribbon's own status colours.
    public let color: UInt32
    public let label: String
}

extension AppModel {
    /// Empty while the hub is unreachable: an old snapshot is not a live mark.
    public var terminalMarks: [TerminalMark] {
        guard store.connectionAvailable else { return [] }
        return TerminalMark.marks(from: store.tabIndicators, colors: settings.statusColors)
    }
}

extension TerminalMark {
    /// In the hub's order, which is its priority. A status this surface does
    /// not know has no colour, and no mark.
    static func marks(from indicators: [HubTabIndicator], colors: StatusColorMap) -> [TerminalMark] {
        indicators.compactMap { item in
            guard let status = Status(rawValue: item.status) else { return nil }
            return TerminalMark(
                host: item.target.host, terminal: item.target.terminal,
                color: colors.color(for: status).hex, label: item.label)
        }
    }
}

extension RGBColor {
    var hex: UInt32 {
        func byte(_ unit: Double) -> UInt32 { UInt32((min(1, max(0, unit)) * 255).rounded()) }
        return byte(r) << 16 | byte(g) << 8 | byte(b)
    }
}
