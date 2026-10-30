import AppKit
import SwiftUI

/// One size for every panel / chrome icon: 14pt glyph in a 28pt hit target.
enum PanelChrome {
    static let symbolSize: CGFloat = 14
    static let hit: CGFloat = 28

    static func symbol(_ systemName: String, weight: Font.Weight = .medium) -> some View {
        Image(systemName: systemName)
            .font(.system(size: symbolSize, weight: weight))
            .symbolRenderingMode(.hierarchical)
    }
}

/// Borderless, icon-only control. Title lives in the help tag and VoiceOver.
struct PanelIconButton: View {
    let systemName: String
    var help: String
    var accessibilityLabel: String
    var accessibilityHint: String = ""
    var emphasized: Bool = false
    var enabled: Bool = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            PanelChrome.symbol(systemName)
                .foregroundStyle(emphasized ? Color.red : Color.primary.opacity(enabled ? 0.75 : 0.28))
                .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .help(help)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(accessibilityHint)
        .accessibilityAddTraits(.isButton)
    }
}

private struct PanelConfirmation: Identifiable {
    let actionId: String
    let jobId: String
    let title: String
    let producer: String

    var id: String { "\(jobId):\(actionId)" }
}

/// Status list shown under the menu-bar ribbon.
/// All visible chrome and controls are rendered by SwiftUI.
struct StatusPanelView: View {
    @Environment(JobStore.self) private var store
    @Environment(SettingsStore.self) private var settings
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    @Environment(\.compactPanel) private var compactPanel
    @FocusState private var listFocused: Bool
    @State private var showingSetup = false
    @State private var expandedId: String?
    @State private var confirmation: PanelConfirmation?
    @State private var resizeOrigin: CGSize?
    @State private var livePanelSize: CGSize?
    /// Pins MenuBarExtra window top-left while dragging so the panel grows down/right
    /// instead of re-centering under the status item (the classic “runs away” bug).
    @State private var resizeAnchor = PanelResizeAnchor()

    /// User preference + system Reduce Motion.
    private var animate: Bool { settings.effectiveAnimationsEnabled && !systemReduceMotion }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if !store.connectionAvailable, !store.activeJobs.isEmpty {
                Label("Connection interrupted · showing last received status", systemImage: "wifi.slash")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(10)
            }
            if let confirmation {
                confirmationBar(confirmation)
            }
            thinDivider
            statusBody
        }
        .modifier(PanelSizing(compact: compactPanel, size: renderedPanelSize))
        .overlay(alignment: .bottomTrailing) {
            if !compactPanel {
            // Three diagonal lines — classic corner resize grip (not bidirectional arrows).
            Image(systemName: "line.3.horizontal")
                .font(.system(size: 9, weight: .semibold))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(.tertiary)
                .rotationEffect(.degrees(-45))
                .frame(width: 24, height: 24)
                .contentShape(Rectangle())
                .gesture(resizeGesture)
                .help("Resize panel")
                .accessibilityLabel("Resize panel")
            }
        }
        // Host window bridge: re-pins origin after every layout pass during resize.
        // A host that fits the panel to its rows has its own window.
        .background { if !compactPanel { PanelWindowBridge(anchor: resizeAnchor) } }
        // Keep ↑/↓ keyboard focus without the system blue focus ring around the panel.
        .focusable()
        .focusEffectDisabled()
        .focused($listFocused)
        .onAppear { listFocused = true }
        .onChange(of: store.selectedJobId) { _, id in
            if let id { expandedId = id }
        }
        .onKeyPress(.downArrow) {
            moveFocus(1)
            return .handled
        }
        .onKeyPress(.upArrow) {
            moveFocus(-1)
            return .handled
        }
        .onKeyPress(.return) {
            toggleFocused()
            return .handled
        }
        .onKeyPress(.space) {
            toggleFocused()
            return .handled
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Nerve status panel")
        // Destructive row actions confirm inline. SwiftUI `.alert` / `.confirmationDialog`
        // often fail to present inside MenuBarExtra `.window`. Clear does not ask.
        .onAppear { store.panelOpen = true }
        .onDisappear {
            store.panelOpen = false
            confirmation = nil
            resizeAnchor.endResize()
            resizeOrigin = nil
            livePanelSize = nil
        }
    }

    private var renderedPanelSize: CGSize {
        livePanelSize ?? CGSize(
            width: CGFloat(SettingsStore.clampPanelWidth(settings.panelWidth)),
            height: CGFloat(SettingsStore.clampPanelHeight(settings.panelHeight))
        )
    }

    private var resizeGesture: some Gesture {
        DragGesture(minimumDistance: 1, coordinateSpace: .global)
            .onChanged { value in
                let origin = resizeOrigin ?? renderedPanelSize
                if resizeOrigin == nil {
                    resizeOrigin = origin
                    resizeAnchor.beginResize()
                }
                let next = CGSize(
                    width: CGFloat(SettingsStore.clampPanelWidth(
                        Double(origin.width + value.translation.width)
                    )),
                    height: CGFloat(SettingsStore.clampPanelHeight(
                        Double(origin.height + value.translation.height)
                    ))
                )
                // No implicit animation — size must track the cursor 1:1.
                var transaction = Transaction()
                transaction.disablesAnimations = true
                withTransaction(transaction) {
                    livePanelSize = next
                }
                // Immediate pin + layout-pass pin (bridge) fights MenuBarExtra re-anchor.
                resizeAnchor.pinTopLeft(contentSize: next)
            }
            .onEnded { _ in
                if let livePanelSize {
                    settings.panelWidth = Double(livePanelSize.width)
                    settings.panelHeight = Double(livePanelSize.height)
                }
                resizeAnchor.endResize()
                resizeOrigin = nil
                // Keep live size one frame so clearing doesn't flash default before
                // settings-backed size is rendered.
                var transaction = Transaction()
                transaction.disablesAnimations = true
                withTransaction(transaction) {
                    self.livePanelSize = nil
                }
            }
    }

    private var listedSubjects: [Job] {
        store.flatStatusJobs(mode: settings.panelGroupMode)
    }

    private var thinDivider: some View {
        Divider()
            .accessibilityHidden(true)
    }

    private func moveFocus(_ delta: Int) {
        let list = listedSubjects
        guard !list.isEmpty else { return }
        let current = list.firstIndex(where: { $0.id == store.selectedJobId }) ?? (delta > 0 ? -1 : list.count)
        let next = min(max(0, current + delta), list.count - 1)
        store.focusedListIndex = next
        store.selectedJobId = list[next].id
    }

    private func toggleFocused() {
        let list = listedSubjects
        guard let job = list.first(where: { $0.id == store.selectedJobId }) ?? list.first else { return }
        let id = job.id
        withAnimation(animate ? .easeInOut(duration: 0.14) : nil) {
            if expandedId == id { expandedId = nil }
            else {
                expandedId = id
                store.selectedJobId = id
            }
        }
    }

    /// Icon-only pull-down. The menu itself is the system menu (checkmark on the
    /// current mode). A caption label plus the menu chevron sat beside the other
    /// 28pt icons and read as a second, mismatched control.
    private var groupModeButton: some View {
        @Bindable var settings = settings
        return Menu {
            Picker("Group by", selection: $settings.panelGroupMode) {
                ForEach(PanelGroupMode.allCases) { mode in
                    Text(mode.title).tag(mode)
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            PanelChrome.symbol("line.3.horizontal.decrease")
                .foregroundStyle(Color.primary.opacity(0.75))
                .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                .contentShape(Rectangle())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .frame(width: PanelChrome.hit, height: PanelChrome.hit)
        .help("Group by \(settings.panelGroupMode.title)")
        .accessibilityLabel("Group by")
        .accessibilityValue(settings.panelGroupMode.title)
    }

    private var refreshButton: some View {
        headerIconButton(
            systemName: "arrow.clockwise",
            help: "Ask the hub to reap dead jobs and republish",
            accessibilityLabel: "Refresh",
            accessibilityHint: "Runs hub maintenance and reloads the job list"
        ) {
            refreshPresentation()
        }
    }

    private var clearButton: some View {
        headerIconButton(
            systemName: "trash",
            help: "Clear all jobs from memory",
            accessibilityLabel: "Clear all",
            accessibilityHint: "Removes all jobs from the panel"
        ) {
            expandedId = nil
            confirmation = nil
            store.clearAll()
        }
        .disabled(store.activeJobs.isEmpty)
    }

    private func headerIconButton(
        systemName: String,
        help: String,
        accessibilityLabel: String,
        accessibilityHint: String,
        emphasized: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        PanelIconButton(
            systemName: systemName,
            help: help,
            accessibilityLabel: accessibilityLabel,
            accessibilityHint: accessibilityHint,
            emphasized: emphasized,
            action: action
        )
    }

    private func refreshPresentation() {
        store.refreshFromHub()
    }

    /// Confirmation lives in the panel — system alerts are unreliable in MenuBarExtra windows.
    @ViewBuilder
    private func confirmationBar(_ item: PanelConfirmation) -> some View {
        HStack(spacing: 10) {
            PanelChrome.symbol("exclamationmark.triangle.fill", weight: .semibold)
                .foregroundStyle(.orange)
                .frame(width: PanelChrome.hit, height: PanelChrome.hit)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 1) {
                Text(confirmationTitle(item))
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                Text(confirmationMessage(item))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            PanelIconButton(
                systemName: "xmark",
                help: "Cancel",
                accessibilityLabel: "Cancel"
            ) {
                confirmation = nil
            }
            .keyboardShortcut(.cancelAction)

            PanelIconButton(
                systemName: "checkmark",
                help: "Confirm",
                accessibilityLabel: "Confirm",
                emphasized: true
            ) {
                performConfirmed(item)
            }
            .keyboardShortcut(.defaultAction)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color.primary.opacity(colorScheme == .dark ? 0.08 : 0.05))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(confirmationTitle(item))
    }

    private func confirmationTitle(_ item: PanelConfirmation) -> String {
        item.title
    }

    private func confirmationMessage(_ item: PanelConfirmation) -> String {
        "Run “\(item.title)”? Provided by \(item.producer)."
    }

    private func performConfirmed(_ item: PanelConfirmation) {
        _ = store.performAction(actionId: item.actionId, jobId: item.jobId, confirmed: true)
        confirmation = nil
    }

    /// Twin metric chips — counts and colors both from `Job.status`
    /// (same source as ribbon bands and row dots). Not raw active/attention facets.
    private var header: some View {
        HStack(spacing: 10) {
            headerMetric(
                systemName: "play.circle.fill",
                count: store.runningCount,
                status: .running,
                help: "\(store.runningCount) running",
                accessibilityLabel: "\(store.runningCount) running"
            )

            if store.attentionCount > 0 {
                headerMetric(
                    systemName: "exclamationmark.circle.fill",
                    count: store.attentionCount,
                    status: .attention,
                    help: "\(store.attentionCount) need attention",
                    accessibilityLabel: "\(store.attentionCount) attention"
                )
            }

            Spacer(minLength: 8)

            HStack(spacing: 4) {
                groupModeButton
                refreshButton
                clearButton
            }
        }
        .padding(.leading, compactPanel ? 8 : 12)
        .padding(.trailing, compactPanel ? 6 : 10)
        .frame(height: compactPanel ? PanelChrome.hit : PanelChrome.hit + 12)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Status")
        .accessibilityAddTraits(.isHeader)
    }

    /// Shared chip: filled-circle SF Symbol + monospaced count.
    /// Color comes from the same status palette as the ribbon / row dots.
    private func headerMetric(
        systemName: String,
        count: Int,
        status: Status,
        help: String,
        accessibilityLabel: String
    ) -> some View {
        let color = RibbonPalette.color(
            for: status,
            scheme: colorScheme,
            map: settings.statusColors
        )
        return HStack(spacing: 2) {
            PanelChrome.symbol(systemName, weight: .semibold)
                .frame(width: PanelChrome.hit, height: PanelChrome.hit)
            Text("\(count)")
                .font((compactPanel ? Font.callout : Font.body).weight(.semibold).monospacedDigit())
        }
        .foregroundStyle(color)
        .help(help)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel)
    }

    @ViewBuilder
    private var statusBody: some View {
        let sections = store.statusSections(mode: settings.panelGroupMode)
        if sections.isEmpty {
            emptyState
        } else if compactPanel {
            // A short list is the list. The scroll view is only for what does not fit.
            // `LazyVStack` reports no height, so the fitting pass uses a real stack.
            ViewThatFits(in: .vertical) {
                sectionList(sections, lazy: false)
                ScrollView { sectionList(sections, lazy: true) }
                    .scrollIndicators(.automatic)
            }
        } else {
            ScrollView { sectionList(sections, lazy: true) }
                .scrollIndicators(.automatic)
        }
    }

    private func sectionList(_ sections: [StatusSection], lazy: Bool) -> some View {
        let rows = sectionsRow(sections)
        return Group {
            if lazy {
                LazyVStack(alignment: .leading, spacing: 0) { rows }
            } else {
                VStack(alignment: .leading, spacing: 0) { rows }
            }
        }
        .padding(.bottom, compactPanel ? 2 : 6)
    }

    @ViewBuilder
    private func sectionsRow(_ sections: [StatusSection]) -> some View {
        ForEach(sections) { section in
            sectionHeader(section.title)
            ForEach(section.jobs) { job in
                let children = job.isGroupJob ? store.members(of: job.id) : []
                let treeOpen = expandedId == job.id
                    || children.contains(where: { $0.id == expandedId })
                ExpandableSubjectRow(
                    subject: job,
                    depth: 0,
                    children: children,
                    isExpanded: treeOpen,
                    isKeyboardFocused: store.selectedJobId == job.id,
                    timeline: store.timeline(for: job.id),
                    childExpandedId: expandedId,
                    childTimelines: Dictionary(
                        uniqueKeysWithValues: children.map {
                            ($0.id, store.timeline(for: $0.id))
                        }
                    ),
                    onToggle: {
                        withAnimation(animate ? .easeInOut(duration: 0.14) : nil) {
                            if treeOpen {
                                expandedId = nil
                            } else {
                                expandedId = job.id
                                store.selectedJobId = job.id
                                if let idx = listedSubjects.firstIndex(where: { $0.id == job.id }) {
                                    store.focusedListIndex = idx
                                }
                            }
                        }
                    },
                    onToggleChild: { childId in
                        withAnimation(animate ? .easeInOut(duration: 0.14) : nil) {
                            if expandedId == childId {
                                expandedId = job.id // collapse child detail; keep tree open
                            } else {
                                expandedId = childId
                                store.selectedJobId = childId
                            }
                        }
                    },
                    onJump: { reveal(job) },
                    onJumpChild: { reveal($0) },
                    onAction: { actionId in
                        handleAction(actionId: actionId, subject: job)
                    },
                    onChildAction: { actionId, child in
                        handleAction(actionId: actionId, subject: child)
                    }
                )
                rowDivider
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            if let error=store.hubError {Text(error).font(.caption).foregroundStyle(.red).multilineTextAlignment(.center)}
            Image(systemName: store.connectionAvailable ? "waveform.path.ecg" : "wifi.slash")
                .font(.largeTitle).foregroundStyle(.secondary)
            Text(store.connectionAvailable ? (store.hasSeenJobs ? "No active conversations" : "Connect your first agent")
                 : (store.lastReceivedAt == nil ? "Connecting to Nerve…" : "Connection interrupted"))
                .font(.headline)
            Text(store.connectionAvailable
                 ? (store.hasSeenJobs ? "New conversations will appear here automatically." : "Connect your agent, then start a conversation to see its status here.")
                 : "Reconnecting automatically. Your agents can keep working.")
                .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
            if store.connectionAvailable {
                Button("Connect an agent") { showingSetup = true }
                    .popover(isPresented: $showingSetup) { AgentSetupGuide().padding(20) }
            } else {
                Button("Retry now") { store.refreshRequestSink?() }
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private func sectionHeader(_ title: String) -> some View {
        // Active and Turn complete are the dot. A machine name is not.
        if !StatusPhrase.isStatus(title) {
            Text(title)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)
                .padding(.horizontal, compactPanel ? 10 : 12)
                .padding(.top, compactPanel ? 2 : 10)
                .padding(.bottom, compactPanel ? 1 : 3)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var rowDivider: some View {
        Divider()
            .padding(.leading, 36)
            .accessibilityHidden(true)
    }

    private func reveal(_ subject: Job) {
        store.selectedJobId = subject.id
        _ = ActionService.openLocation(subject)
    }

    private func handleAction(actionId: String, subject: Job) {
        guard let action = subject.actions.first(where: { $0.id == actionId }) else { return }
        if ActionService.isDestructive(action) {
            confirmation = PanelConfirmation(
                actionId: actionId,
                jobId: subject.id,
                title: action.title,
                producer: subject.producer.name ?? subject.producer.id
            )
        } else {
            _ = store.performAction(actionId: actionId, jobId: subject.id, confirmed: true)
        }
    }
}

public struct StatusPanelRoot: View {
    @Bindable var store: JobStore
    @Bindable var settings: SettingsStore
    /// Hug the rows. The menu-bar window keeps the size the person dragged.
    var fitsContent: Bool

    public init(store: JobStore, settings: SettingsStore, fitsContent: Bool = false) {
        self.store = store
        self.settings = settings
        self.fitsContent = fitsContent
    }

    public var body: some View {
        StatusPanelView()
            .environment(store)
            .environment(settings)
            .environment(\.compactPanel, fitsContent)
    }
}

/// A hosted panel is as tall as its rows, and no taller than the saved height.
private struct PanelSizing: ViewModifier {
    var compact: Bool
    var size: CGSize

    func body(content: Content) -> some View {
        if compact {
            content
                .frame(width: size.width, alignment: .topLeading)
                .frame(maxHeight: size.height, alignment: .top)
        } else {
            content.frame(width: size.width, height: size.height, alignment: .topLeading)
        }
    }
}

private struct CompactPanelKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    var compactPanel: Bool {
        get { self[CompactPanelKey.self] }
        set { self[CompactPanelKey.self] = newValue }
    }
}
