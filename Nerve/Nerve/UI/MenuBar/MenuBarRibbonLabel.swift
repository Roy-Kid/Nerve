import AppKit
import NerveRibbonUI
import SwiftUI

// MARK: - Menu-bar ribbon ambient clock

/// Owns the RunLoop timer so a SwiftUI `View` value type never captures `@State`.
@MainActor
final class RibbonAmbientClock: ObservableObject {
    @Published private(set) var tick: UInt64 = 0
    private var timer: Timer?

    var phase: TimeInterval { Double(tick) / 20.0 }

    func setActive(_ active: Bool) {
        if active {
            guard timer == nil else { return }
            let timer = Timer(timeInterval: 1.0 / 20.0, repeats: true) { [weak self] _ in
                Task { @MainActor in
                    self?.tick &+= 1
                }
            }
            RunLoop.main.add(timer, forMode: .common)
            self.timer = timer
        } else {
            timer?.invalidate()
            timer = nil
        }
    }

    isolated deinit {
        timer?.invalidate()
    }
}

// MARK: - Menu-bar ribbon label

/// MenuBarExtra label — pure SwiftUI `Canvas` (always visible) + class-owned timer
/// for ambient frames. `NSViewRepresentable` in the status item often paints blank.
struct MenuBarRibbonLabel: View {
    @Bindable var store: JobStore
    @Bindable var settings: SettingsStore
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var clock = RibbonAmbientClock()

    private var animate: Bool { settings.effectiveAnimationsEnabled }

    private var ambientActive: Bool {
        animate
            && settings.ribbonMotionStyle.usesAmbientMotion
            && store.activeCount > 0
            && RibbonRenderer.ambientRelevant(store: store, settings: settings)
    }

    private var motion: RibbonMotionStyle {
        ambientActive ? settings.ribbonMotionStyle : .transitionsOnly
    }

    private var phase: TimeInterval { ambientActive ? clock.phase : 0 }

    var body: some View {
        // Observe ambient ticks while motion is on, and every hub write so
        // MenuBarExtra cannot keep a frozen ribbon after a frame arrives.
        let _ = clock.tick
        let _ = store.revision
        ribbonImage
            // Explicit frame keeps MenuBarExtra from collapsing a zero-size label.
            // Reserve a stable slot while visible and pin the ribbon to its
            // right edge, so it expands left without moving its menu-bar anchor.
            .frame(width: slotWidth, height: 22, alignment: .trailing)
            .animation(animate ? .easeInOut(duration: 0.28) : nil, value: visualSignature)
            .contentShape(Rectangle())
            .accessibilityLabel("Nerve")
            .accessibilityValue("\(store.activeCount) active jobs")
            .onAppear { clock.setActive(ambientActive) }
            .onDisappear { clock.setActive(false) }
            .onChange(of: ambientActive) { _, on in clock.setActive(on) }
    }

    /// Same `Image(size:)` path that painted the ribbon before the NSView attempt.
    private var ribbonImage: Image {
        let width = max(14, preferredWidth)
        let height: CGFloat = 22
        let barHeight = min(18, max(3, CGFloat(settings.ribbonThickness)))
        let rect = CGRect(
            x: 2,
            y: (height - barHeight) / 2,
            width: max(6, width - 4),
            height: barHeight
        )
        let path = Path(roundedRect: rect, cornerRadius: barHeight / 2, style: .continuous)
        let stops = ribbonStops(motion: motion, time: phase)
        let shimmerCycle = shimmerOverlay(motion: motion, time: phase)
        let dark = colorScheme == .dark
        // Include tick in the label so Image identity refreshes each ambient frame.
        let label = Text("Nerve ribbon \(clock.tick)")

        return Image(
            size: CGSize(width: width, height: height),
            label: label,
            opaque: false,
            colorMode: .nonLinear
        ) { context in
            context.fill(
                path,
                with: .linearGradient(
                    Gradient(stops: stops),
                    startPoint: CGPoint(x: rect.minX, y: rect.midY),
                    endPoint: CGPoint(x: rect.maxX, y: rect.midY)
                )
            )
            if let cycle = shimmerCycle {
                context.drawLayer { layer in
                    layer.clip(to: path)
                    let travel = rect.width + rect.width * 0.7
                    let centerX = rect.minX - rect.width * 0.35 + CGFloat(cycle) * travel
                    // Original wider glint (~48% of bar), soft white so color still reads.
                    let half = max(10, rect.width * 0.24)
                    let shine = Path(CGRect(
                        x: centerX - half,
                        y: rect.minY,
                        width: half * 2,
                        height: rect.height
                    ))
                    // Keep this a veil, never a solid wash (peak << 0.3).
                    let peak = dark ? 0.20 : 0.14
                    layer.fill(
                        shine,
                        with: .linearGradient(
                            Gradient(stops: [
                                .init(color: .white.opacity(0), location: 0),
                                .init(color: .white.opacity(peak * 0.4), location: 0.32),
                                .init(color: .white.opacity(peak), location: 0.5),
                                .init(color: .white.opacity(peak * 0.4), location: 0.68),
                                .init(color: .white.opacity(0), location: 1),
                            ]),
                            startPoint: CGPoint(x: centerX - half, y: rect.midY),
                            endPoint: CGPoint(x: centerX + half, y: rect.midY)
                        )
                    )
                }
            }
            context.stroke(path, with: .color(Color.primary.opacity(0.22)), lineWidth: 0.5)
        }
        .renderingMode(.original)
    }

    private var preferredWidth: CGFloat {
        let count = store.activeCount
        if count == 0 {
            return max(14, (22 * CGFloat(settings.ribbonLengthScale)).rounded())
        }
        let base = 28 + (100 - 28) * store.ribbonLengthFactor()
        // Match the AppKit renderer's cap so the menu-bar slot stays compact
        // even when users choose the largest ribbon scale.
        return min(120, max(16, base * CGFloat(settings.ribbonLengthScale))).rounded()
    }

    private var slotWidth: CGFloat {
        store.activeCount == 0 ? preferredWidth : 120
    }

    private var visualSignature: String {
        "\(store.ribbonSignature(mode: settings.panelGroupMode))|\(settings.ribbonLengthScale)|\(settings.ribbonThickness)|\(settings.statusColors)|\(settings.ribbonMotionStyle.rawValue)"
    }

    private func ribbonStops(motion: RibbonMotionStyle, time: TimeInterval) -> [Gradient.Stop] {
        let segments = store.ribbonSegments(mode: settings.panelGroupMode)
        guard !segments.isEmpty else {
            let idle = baseStatusColor(.inactive).opacity(0.55)
            return [
                .init(color: idle, location: 0),
                .init(color: idle, location: 1),
            ]
        }

        var stops: [Gradient.Stop] = []
        var cursor: CGFloat = 0
        for (index, segment) in segments.enumerated() {
            let color = animatedColor(for: segment.status, motion: motion, time: time)
            let end = min(1, cursor + segment.weight)
            if index == 0 {
                stops.append(.init(color: color, location: 0))
            }
            if index < segments.count - 1 {
                let next = segments[index + 1]
                let nextColor = animatedColor(for: next.status, motion: motion, time: time)
                let blend = min(
                    NerveRibbonRenderer.maxBlend,
                    min(segment.weight, next.weight) * NerveRibbonRenderer.neighborBlend
                )
                stops.append(.init(color: color, location: max(cursor, end - blend)))
                stops.append(.init(color: nextColor, location: min(1, end + blend)))
            } else {
                stops.append(.init(color: color, location: 1))
            }
            cursor = end
        }
        return stops
    }

    /// Progress 0…1 of the shimmer cycle, or nil when shimmer is off.
    private func shimmerOverlay(motion: RibbonMotionStyle, time: TimeInterval) -> Double? {
        guard motion == .shimmer || motion == .full else { return nil }
        let period = NerveRibbonRenderer.shimmerPeriod
        return (time.truncatingRemainder(dividingBy: period)) / period
    }

    private func animatedColor(
        for status: Status,
        motion: RibbonMotionStyle,
        time: TimeInterval
    ) -> Color {
        let base = baseStatusColor(status)
        let pulse = pulseAmount(for: status, motion: motion, time: time)
        guard pulse > 0.001 else { return base }
        // Hard cap: never wash a segment to white (was 0.4–0.6 → full bleach).
        return lift(base, towardWhite: min(0.16, pulse))
    }

    private func pulseAmount(
        for status: Status,
        motion: RibbonMotionStyle,
        time: TimeInterval
    ) -> CGFloat {
        func wave(period: TimeInterval, amplitude: CGFloat) -> CGFloat {
            guard period > 0 else { return 0 }
            let p = (time.truncatingRemainder(dividingBy: period)) / period
            // sin in [-1,1] → 0…1, scaled by amplitude (keep amplitudes small).
            return amplitude * CGFloat((sin(p * 2 * Double.pi - Double.pi / 2) + 1) / 2)
        }
        switch motion {
        case .transitionsOnly, .shimmer:
            return 0
        case .breathe:
            switch status {
            case .running, .waiting: return wave(period: 2.2, amplitude: 0.14)
            case .monitor: return wave(period: 2.8, amplitude: 0.10)
            default: return 0
            }
        case .statusPulse, .full:
            // Full mode also runs shimmer — keep body pulse subtle so they don't stack to white.
            switch status {
            case .running, .waiting: return wave(period: 2.2, amplitude: 0.12)
            case .monitor: return wave(period: 2.8, amplitude: 0.10)
            case .attention: return wave(period: 1.4, amplitude: 0.16)
            case .problem: return wave(period: 0.9, amplitude: 0.18)
            default: return 0
            }
        }
    }

    private func baseStatusColor(_ status: Status) -> Color {
        let rgb = settings.statusColors.color(for: status)
        let lift = colorScheme == .dark ? 0.04 : 0
        return Color(
            red: rgb.r + (1 - rgb.r) * lift,
            green: rgb.g + (1 - rgb.g) * lift,
            blue: rgb.b + (1 - rgb.b) * lift
        )
    }

    private func lift(_ color: Color, towardWhite amount: CGFloat) -> Color {
        let a = min(1, max(0, amount))
        let ns = NSColor(color).usingColorSpace(.sRGB) ?? NSColor(color)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, alpha: CGFloat = 0
        ns.getRed(&r, green: &g, blue: &b, alpha: &alpha)
        return Color(
            red: r + (1 - r) * a,
            green: g + (1 - g) * a,
            blue: b + (1 - b) * a,
            opacity: alpha
        )
    }
}
