import AppKit

/// A weighted colour band. Adjacent bands blend at their boundary.
public struct NerveRibbonSegment: Sendable, Hashable {
  public let color: UInt32
  public let weight: CGFloat
  public let lift: CGFloat

  public init(color: UInt32, weight: CGFloat, lift: CGFloat = 0) {
    self.color = color
    self.weight = max(0.0001, weight)
    self.lift = min(0.16, max(0, lift))
  }
}

/// The AppKit renderer behind the Nerve menu-bar ribbon.
public enum NerveRibbonRenderer {
  /// Cap on the blend width between two segments. Same number the live
  /// SwiftUI ribbon uses (`MenuBarRibbonLabel`).
  public static let maxBlend: CGFloat = 0.045
  /// How much of the narrower neighbour may bleed into the ramp.
  public static let neighborBlend: CGFloat = 0.28
  /// Seconds for one shimmer pass.
  public static let shimmerPeriod: TimeInterval = 1.8

  public static func image(
    segments: [NerveRibbonSegment],
    width: CGFloat,
    height: CGFloat = 22,
    thickness: CGFloat = 8,
    idleColor: UInt32 = 0x8E8E93,
    appearance: NSAppearance,
    time: TimeInterval = 0,
    shimmer: Bool = false
  ) -> NSImage {
    let size = NSSize(width: max(1, width), height: max(1, height))
    let image = NSImage(size: size)
    image.lockFocus()
    defer { image.unlockFocus() }

    NSGraphicsContext.current?.imageInterpolation = .high
    appearance.performAsCurrentDrawingAppearance {
      let dark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
      let barHeight = min(size.height - 4, max(2, thickness))
      let rect = NSRect(x: 2, y: (size.height - barHeight) / 2,
                        width: max(6, size.width - 4), height: barHeight)
      let path = NSBezierPath(roundedRect: rect, xRadius: barHeight / 2, yRadius: barHeight / 2)
      NSGraphicsContext.saveGraphicsState()
      path.addClip()

      if let gradient = gradient(segments: segments, dark: dark, idle: idleColor) {
        gradient.draw(in: rect, angle: 0)
      } else {
        resolve(idleColor, dark: dark, lift: 0).withAlphaComponent(0.55).setFill()
        rect.fill()
      }
      if shimmer { drawShimmer(in: rect, time: time, dark: dark) }
      NSGraphicsContext.restoreGraphicsState()

      NSColor.separatorColor.withAlphaComponent(dark ? 0.45 : 0.30).setStroke()
      path.lineWidth = 0.5
      path.stroke()
    }
    return image
  }

  private static func gradient(segments: [NerveRibbonSegment], dark: Bool, idle: UInt32) -> NSGradient? {
    guard !segments.isEmpty else { return nil }
    func color(_ segment: NerveRibbonSegment) -> NSColor {
      resolve(segment.color, dark: dark, lift: segment.lift)
    }
    if segments.count == 1 {
      let c = color(segments[0])
      return NSGradient(colors: [c, c])
    }

    let total = segments.reduce(CGFloat.zero) { $0 + $1.weight }
    let weights = segments.map { $0.weight / total }
    let blend = maxBlend
    var stops: [(NSColor, CGFloat)] = []
    var cursor: CGFloat = 0
    for (index, segment) in segments.enumerated() {
      let start = cursor
      let end = cursor + weights[index]
      let incoming = index == 0 ? 0 : min(blend, min(weights[index], weights[index - 1]) * neighborBlend)
      let outgoing = index == segments.count - 1 ? 0 : min(blend, min(weights[index], weights[index + 1]) * neighborBlend)
      let pureStart = min(end, start + incoming * 0.5)
      let pureEnd = max(pureStart, end - outgoing * 0.5)
      let current = color(segment)
      if index == 0 { stops.append((current, 0)) }
      stops.append((current, pureStart))
      stops.append((current, pureEnd))
      if index < segments.count - 1 {
        stops.append((current, max(pureEnd, end - outgoing * 0.5)))
        stops.append((color(segments[index + 1]), min(1, end + outgoing * 0.5)))
      } else {
        stops.append((current, 1))
      }
      cursor = end
    }

    var cleaned: [(NSColor, CGFloat)] = []
    for stop in stops.sorted(by: { $0.1 < $1.1 }) {
      let location = min(1, max(0, stop.1))
      if let last = cleaned.last, abs(last.1 - location) < 0.004 {
        cleaned[cleaned.count - 1] = (stop.0, location)
      } else {
        cleaned.append((stop.0, location))
      }
    }
    guard cleaned.count > 1 else {
      let c = cleaned.first?.0 ?? resolve(idle, dark: dark, lift: 0)
      return NSGradient(colors: [c, c])
    }
    let colors = cleaned.map(\.0)
    var locations = cleaned.map(\.1)
    return locations.withUnsafeMutableBufferPointer { buffer in
      NSGradient(colors: colors, atLocations: buffer.baseAddress!, colorSpace: .sRGB)
    }
  }

  private static func resolve(_ hex: UInt32, dark: Bool, lift: CGFloat) -> NSColor {
    let r = CGFloat((hex >> 16) & 0xFF) / 255
    let g = CGFloat((hex >> 8) & 0xFF) / 255
    let b = CGFloat(hex & 0xFF) / 255
    let base = NSColor(srgbRed: r, green: g, blue: b, alpha: 1)
    let amount = min(0.20, lift + (dark ? 0.04 : 0))
    return base.blended(withFraction: amount, of: .white) ?? base
  }

  private static func drawShimmer(in rect: NSRect, time: TimeInterval, dark: Bool) {
    let cycle = time.truncatingRemainder(dividingBy: shimmerPeriod) / shimmerPeriod
    let center = rect.minX - rect.width * 0.3 + cycle * (rect.width * 1.6)
    let half = max(10, rect.width * 0.24)
    let peak: CGFloat = dark ? 0.20 : 0.14
    guard let gradient = NSGradient(colorsAndLocations:
      (.white.withAlphaComponent(0), 0),
      (.white.withAlphaComponent(peak * 0.4), 0.32),
      (.white.withAlphaComponent(peak), 0.5),
      (.white.withAlphaComponent(peak * 0.4), 0.68),
      (.white.withAlphaComponent(0), 1)
    ) else { return }
    gradient.draw(in: NSRect(x: center - half, y: rect.minY, width: half * 2, height: rect.height), angle: 0)
  }
}
