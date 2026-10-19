import AppKit
import SwiftUI

// MARK: - Status color dot

/// Native semantic status marker with the standard delayed help tip.
private struct StatusColorDot: View {
    let status: Status
    let color: Color

    var body: some View {
        Circle()
            .fill(color)
            .overlay {
                Circle()
                    .strokeBorder(Color.primary.opacity(0.12), lineWidth: 0.5)
            }
            .frame(width: PanelChrome.symbolSize, height: PanelChrome.symbolSize)
            .frame(width: PanelChrome.hit, height: PanelChrome.hit)
            .contentShape(Rectangle())
            .help(status.title)
            .accessibilityLabel(status.title)
            .accessibilityAddTraits(.isStaticText)
    }
}

// MARK: - Single-line expandable subject (tree: subtasks + details)

struct ExpandableSubjectRow: View {
    let subject: Job
    /// Indent level (0 = root, 1 = subtask under a group).
    var depth: Int = 0
    /// Direct children (group members). Empty for leaves / standalone jobs.
    var children: [Job] = []
    let isExpanded: Bool
    var isKeyboardFocused: Bool = false
    var timeline: [TimelineEntry]
    /// Which child id is expanded for detail (parent may stay expanded for tree).
    var childExpandedId: String? = nil
    var childTimelines: [String: [TimelineEntry]] = [:]
    var onToggle: () -> Void
    var onToggleChild: ((String) -> Void)? = nil
    var onAction: (String) -> Void
    var onChildAction: ((String, Job) -> Void)? = nil

    @Environment(\.colorScheme) private var colorScheme
    @Environment(SettingsStore.self) private var settings
    @State private var hovered = false

    private var indent: CGFloat { CGFloat(depth) * 14 }

    private var hasSubtasks: Bool { !children.isEmpty || subject.isGroupJob }

    private var visibleColumns: [PanelColumn] { settings.panelColumns }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                // Tree disclosure + status
                if depth == 0 && hasSubtasks {
                    PanelChrome.symbol(isExpanded ? "chevron.down" : "chevron.right", weight: .semibold)
                        .foregroundStyle(.tertiary)
                        .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                        .accessibilityHidden(true)
                } else if depth > 0 {
                    PanelChrome.symbol("arrow.turn.down.right")
                        .foregroundStyle(.quaternary)
                        .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                        .accessibilityHidden(true)
                }

                StatusColorDot(
                    status: subject.status,
                    color: RibbonPalette.color(
                        for: subject.status,
                        scheme: colorScheme,
                        map: settings.statusColors
                    )
                )

                Button(action: onToggle) {
                    HStack(spacing: 8) {
                        ForEach(visibleColumns) { column in
                            columnCell(column)
                        }

                        Text(subject.statusLabel)
                            .font(.caption.weight(.medium))
                            .foregroundStyle(RibbonPalette.color(for: subject.status, scheme: colorScheme, map: settings.statusColors))
                            .lineLimit(1)
                            .frame(width: 76, alignment: .leading)

                        PanelChrome.symbol("chevron.right", weight: .semibold)
                            .foregroundStyle(.quaternary)
                            .rotationEffect(.degrees(isExpanded ? 90 : 0))
                            .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                            .accessibilityHidden(true)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                if subject.status == .attention,
                   let action = subject.actions.first(where: { ActionService.isOpenKind($0.kind) }) {
                    PanelIconButton(
                        systemName: "arrow.up.forward.app",
                        help: ActionService.focusActionTitle(for: subject),
                        accessibilityLabel: ActionService.focusActionTitle(for: subject),
                        enabled: action.state == .available
                    ) {
                        onAction(action.id)
                    }
                } else {
                    // Keep the trailing action column stable so status and disclosure
                    // stay aligned even when only some jobs need attention.
                    Color.clear
                        .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                        .accessibilityHidden(true)
                }
            }
            .padding(.leading, 12 + indent)
            .padding(.trailing, 12)
            .padding(.vertical, 7)
            .background { rowSelectionBackground }
            .onHover { hovered = $0 }
            .animation(nil, value: hovered)
            .accessibilityElement(children: .combine)
            .accessibilityLabel(accessibilityRowLabel)
            .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")
            .accessibilityHint(expandHint)
            .accessibilityAddTraits(isExpanded ? [.isSelected] : [])
            .accessibilityAction { onToggle() }

            if isExpanded {
                expandedContent
                    .padding(.leading, 28 + indent)
                    .padding(.trailing, 10)
                    .padding(.bottom, 9)
                    .transition(.opacity)
            }
        }
    }

    private var expandHint: String {
        if hasSubtasks && depth == 0 {
            return isExpanded ? "Collapse subtasks and details" : "Expand subtasks and details"
        }
        return isExpanded ? "Collapse details" : "Expand details"
    }

    @ViewBuilder
    private var expandedContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            // --- Subtasks (group members) ---
            if !children.isEmpty {
                sectionLabel("Subtasks", count: children.count)
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(children) { child in
                        ExpandableSubjectRow(
                            subject: child,
                            depth: depth + 1,
                            children: [],
                            isExpanded: childExpandedId == child.id,
                            isKeyboardFocused: false,
                            timeline: childTimelines[child.id] ?? [],
                            onToggle: { onToggleChild?(child.id) },
                            onAction: { actionId in
                                onChildAction?(actionId, child)
                            }
                        )
                    }
                }
                .padding(.leading, 4)
                .padding(.vertical, 2)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(Color.primary.opacity(colorScheme == .dark ? 0.06 : 0.03))
                }
            } else if subject.isGroupJob {
                // Group with no visible members under current Settings filter
                Text(emptySubtasksHint)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .padding(.leading, 2)
            }

            // --- Details (metadata / local actions / timeline) ---
            sectionLabel("Details")
            detailBlock
        }
    }

    private var emptySubtasksHint: String {
        switch settings.panelMemberVisibility {
        case .never:
            return "Subtasks hidden (Settings → Batch members: Groups only)"
        case .attention:
            return "No problem/attention subtasks"
        case .all:
            return "No subtasks reported"
        }
    }

    private func sectionLabel(_ title: String, count: Int? = nil) -> some View {
        HStack(spacing: 6) {
            Text(title)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
            if let count {
                Text("\(count)")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.quaternary)
            }
        }
        .padding(.top, 2)
    }

    @ViewBuilder
    private func columnCell(_ column: PanelColumn) -> some View {
        let text = text(for: column)
        Text(text)
            .font(columnFont(column))
            .foregroundStyle(column == .name ? Color.primary : Color.secondary)
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(
                minWidth: column.isFlexible ? 40 : column.width,
                idealWidth: column.isFlexible ? nil : column.width,
                maxWidth: column.isFlexible ? .infinity : column.width,
                alignment: .leading
            )
            .layoutPriority(column.isFlexible ? 0 : 1)
            .help(text.isEmpty ? column.title : text)
            .opacity(text.isEmpty ? 0.35 : 1)
    }

    private func columnFont(_ column: PanelColumn) -> Font {
        switch column {
        case .name: return .body.weight(.medium)
        case .updated, .machine: return .caption2.monospaced()
        case .summary: return .caption
        default: return .caption
        }
    }

    private var accessibilityRowLabel: String {
        ([subject.statusLabel] + visibleColumns.map { text(for: $0) })
            .filter { !$0.isEmpty }
            .joined(separator: ", ")
    }

    private func text(for column: PanelColumn) -> String {
        switch column {
        case .name:
            return subject.name
        case .summary:
            return activityText
        case .machine:
            return subject.alias
        case .updated:
            return timeLabel
        case .producer, .status:
            return "" // retired — see PanelColumn.isRenderable
        }
    }

    private var activityText: String { subject.display?.activity ?? "" }

    @ViewBuilder
    private var rowSelectionBackground: some View {
        if hovered || isExpanded || isKeyboardFocused {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(rowSelectionColor)
                .padding(.horizontal, 6)
        }
    }

    private var rowSelectionColor: Color {
        if isKeyboardFocused {
            return Color.accentColor.opacity(colorScheme == .dark ? 0.30 : 0.14)
        }
        return Color.primary.opacity(colorScheme == .dark ? 0.08 : 0.055)
    }

    private var detailBlock: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 4) {
                if let prompt = subject.lastPrompt {
                    promptBlock(prompt)
                }
                if let summary = subject.current?.summary ?? subject.current?.name, !summary.isEmpty {
                    meta("Doing", summary)
                }
                if let project = subject.context?.project, !project.isEmpty, project != subject.name {
                    meta("Project", project)
                }
                if !subject.alias.isEmpty {
                    meta("Machine", subject.alias)
                }
                meta("Updated", format(subject.updatedAt))

                if !subject.actions.isEmpty {
                    HStack(spacing: 0) {
                        ForEach(subject.actions) { action in
                            actionButton(action)
                        }
                    }
                    .padding(.top, 4)
                }

                if !timeline.isEmpty {
                    Divider()
                        .padding(.vertical, 3)

                    Text("Recent")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.tertiary)

                    ForEach(timeline.prefix(5)) { entry in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(entry.timestamp.formatted(date: .omitted, time: .shortened))
                                .font(.caption2.monospacedDigit())
                                .foregroundStyle(.quaternary)
                                .frame(width: 52, alignment: .leading)
                            Text(entry.title)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                    }
                }
            }
            .padding(2)
        }
        .groupBoxStyle(.automatic)
        .font(.caption)
    }

    /// What was asked, above what is happening: prompts run past one line, so
    /// this gets its own wrapped block rather than a truncated `meta` row.
    private func promptBlock(_ prompt: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("Prompt")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.tertiary)
            Text(prompt)
                .font(.caption)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .lineLimit(4)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.bottom, 2)
    }

    private func meta(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(label)
                .foregroundStyle(.tertiary)
                .frame(width: 56, alignment: .leading)
            Text(value)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private func actionButton(_ action: JobAction) -> some View {
        let isOpen = ActionService.isOpenKind(action.kind)
        let helpText = isOpen
            ? ActionService.focusActionTitle(for: subject)
            : action.title
        PanelIconButton(
            systemName: symbol(for: action),
            help: helpText,
            accessibilityLabel: helpText,
            emphasized: ActionService.isDestructive(action),
            enabled: action.state == .available
        ) {
            onAction(action.id)
        }
    }

    private func symbol(for action: JobAction) -> String {
        let kind = action.kind.lowercased()
        if ActionService.isOpenKind(kind) { return "arrow.up.forward.app" }
        switch kind {
        case "copy", "copy_summary", "copysummary": return "doc.on.doc"
        case "open_logs", "openlogs": return "doc.text"
        case "hide", "mute": return "bell.slash"
        default:
            return ActionService.isDestructive(action) ? "exclamationmark.triangle" : "ellipsis"
        }
    }

    private var timeLabel: String {
        if subject.lifecycle == .ended, let end = subject.endedAt {
            return relative(end)
        }
        if let start = subject.startedAt {
            return relative(start)
        }
        return relative(subject.updatedAt)
    }

    private func relative(_ date: Date) -> String {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .abbreviated
        return f.localizedString(for: date, relativeTo: Date())
    }

    private func format(_ date: Date) -> String {
        date.formatted(date: .abbreviated, time: .shortened)
    }
}
