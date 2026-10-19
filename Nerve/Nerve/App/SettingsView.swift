import AppKit
import SwiftUI

// MARK: - Root

enum PreferencesTab: String, CaseIterable, Identifiable {
    case general
    case machines
    case appearance
    case notifications
    case about

    var id: String { rawValue }

    var title: String {
        switch self {
        case .general: return "General"
        case .machines: return "Machines"
        case .appearance: return "Appearance"
        case .notifications: return "Notifications"
        case .about: return "About"
        }
    }

    var subtitle: String {
        switch self {
        case .general: return "Menu bar, status panel, and local endpoint"
        case .machines: return "This Mac and SSH reverse tunnels"
        case .appearance: return "Ribbon size and status colors"
        case .notifications: return "Alerts, quiet hours, and mutes"
        case .about: return "Version, privacy, and local endpoint"
        }
    }

    var systemImage: String {
        switch self {
        case .general: return "gearshape.fill"
        case .machines: return "desktopcomputer"
        case .appearance: return "paintpalette.fill"
        case .notifications: return "bell.badge.fill"
        case .about: return "info"
        }
    }

    var tint: Color {
        switch self {
        case .general: return Color(nsColor: .systemGray)
        case .machines: return Color(nsColor: .systemTeal)
        case .appearance: return Color(nsColor: .systemIndigo)
        case .notifications: return Color(nsColor: .systemRed)
        case .about: return Color(nsColor: .systemBlue)
        }
    }
}

struct PreferencesView: View {
    @Bindable var store: JobStore
    @Bindable var settings: SettingsStore
    @Bindable var tunnels: MachineTunnelManager
    var onRibbonRefresh: (() -> Void)?

    @State private var tab: PreferencesTab? = .general
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    /// Drives the machine editor sheet. Prefer `sheet(item:)` over `isPresented` + optional —
    /// an empty sheet body freezes the Settings window on macOS.
    @State private var isRefreshingSSH = false

    private var selectedTab: PreferencesTab { tab ?? .general }

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            List(PreferencesTab.allCases, selection: $tab) { item in
                SettingsSidebarLabel(item: item)
                    .tag(item)
                    .accessibilityLabel(item.title)
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: 168, ideal: 184, max: 210)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                sidebarFooter
            }
        } detail: {
            Group {
                switch selectedTab {
                case .general:
                    generalPane
                case .machines:
                    machinesPane
                case .appearance:
                    appearancePane
                case .notifications:
                    notificationsPane
                case .about:
                    aboutPane
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .navigationSplitViewStyle(.balanced)
        .frame(minWidth: 660, idealWidth: 700, minHeight: 500, idealHeight: 540)
    }

    private var sidebarFooter: some View {
        VStack(spacing: 0) {
            Divider()
            HStack(spacing: 8) {
                Image("Logo")
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 18, height: 18)
                    .accessibilityHidden(true)

                Text("Nerve")
                    .font(.caption.weight(.medium))

                Spacer(minLength: 4)

                Text("v\(appVersion)")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
        }
        .background(.ultraThinMaterial)
    }

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
    }

    // MARK: General

    private var generalPane: some View {
        SettingsPage(tab: .general) {
            Form {
                Section {
                    PreferenceToggleRow(
                        title: "Hide when idle",
                        description: "Remove Nerve from the menu bar when nothing is active.",
                        isOn: Binding(
                            get: { settings.hideWhenIdle },
                            set: {
                                settings.hideWhenIdle = $0
                                onRibbonRefresh?()
                            }
                        )
                    )

                    PreferenceToggleRow(
                        title: "Animate status changes",
                        description: "Ease ribbon and panel transitions. Required for continuous ribbon motion.",
                        isOn: $settings.animationsEnabled
                    )

                    Picker(
                        "Ribbon motion",
                        selection: $settings.ribbonMotionStyle
                    ) {
                        ForEach(RibbonMotionStyle.allCases) { style in
                            Text(style.title).tag(style)
                        }
                    }
                    .pickerStyle(.menu)
                    .disabled(!settings.animationsEnabled)
                    .opacity(settings.animationsEnabled ? 1 : 0.55)

                    Text(settings.ribbonMotionStyle.help)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    if settings.systemReduceMotionActive {
                        PreferenceToggleRow(
                            title: "Respect system Reduce Motion",
                            description: "macOS Reduce Motion is on. Keep this enabled to freeze motion, or turn it off to allow ribbon animation anyway.",
                            isOn: $settings.respectSystemReduceMotion
                        )
                    }

                    PreferenceToggleRow(
                        title: "Ribbon shows roots only",
                        description: "Only group and session roots color the menu bar. Turn off to also paint problem members.",
                        isOn: Binding(
                            get: { settings.ribbonRootsOnly },
                            set: {
                                settings.ribbonRootsOnly = $0
                                onRibbonRefresh?()
                            }
                        )
                    )
                } header: {
                    Text("Menu Bar")
                } footer: {
                    Text("Size and colors are under Appearance.")
                }

                Section {
                    Picker(
                        "Group by",
                        selection: Binding(
                            get: { settings.panelGroupMode },
                            set: {
                                settings.panelGroupMode = $0
                                onRibbonRefresh?()
                            }
                        )
                    ) {
                        ForEach(PanelGroupMode.allCases) { mode in
                            Text(mode.title).tag(mode)
                        }
                    }
                    .pickerStyle(.menu)

                    Picker(
                        "Batch members",
                        selection: Binding(
                            get: { settings.panelMemberVisibility },
                            set: {
                                settings.panelMemberVisibility = $0
                                onRibbonRefresh?()
                            }
                        )
                    ) {
                        ForEach(PanelMemberVisibility.allCases) { mode in
                            Text(mode.title).tag(mode)
                        }
                    }
                    .pickerStyle(.menu)

                    Text(settings.panelMemberVisibility.help)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    ForEach(PanelColumn.configurableColumns) { column in
                        Toggle(isOn: Binding(
                            get: { settings.isPanelColumnEnabled(column) },
                            set: { settings.setPanelColumn(column, enabled: $0) }
                        )) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(column.title)
                                Text(column.help)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                } header: {
                    Text("Status Panel")
                } footer: {
                    Text("Grouping also orders the ribbon. Columns keep fixed widths so rows align; hover hides Updated and gives the space to Activity.")
                }

                Section {
                    LabeledContent("Endpoint") {
                        Text("\(NerveEndpoint.host):\(NerveEndpoint.port)")
                            .font(.body.monospaced())
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                } header: {
                    Text("Local Endpoint")
                } footer: {
                    Text("Fixed and loopback only — the port is not configurable, because binding it is what keeps a single nerve-hub running. Jobs live in that hub's memory and clear when it exits. Remotes reach this port via Machines tunnels.")
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
        }
    }

    // MARK: Machines

    private var machinesPane: some View {
        SettingsPage(tab: .machines) {
            Form {
                Section {
                    HStack(spacing: 12) {
                        Image(systemName: "laptopcomputer")
                            .foregroundStyle(Color(nsColor: .systemBlue))
                            .frame(width: 24)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(LocalMachine.alias)
                                .font(.body.weight(.medium))
                            Text("This Mac · always available")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text("Local")
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color(nsColor: .systemGreen).opacity(0.15), in: Capsule())
                            .foregroundStyle(Color(nsColor: .systemGreen))
                    }
                } header: {
                    Text("This Mac")
                } footer: {
                    Text("Local jobs usually report alias “\(LocalMachine.alias)”. Any free-form alias is shown — this page only manages tunnels, not an allow-list.")
                }

                Section {
                    if settings.machines.isEmpty {
                        Text("No SSH hosts.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .padding(.vertical, 4)
                    } else {
                        ForEach(settings.machines) { machine in
                            machineRow(machine)
                        }
                    }
                } header: {
                    HStack {
                        Text("Remote Tunnels")
                        Spacer()
                        Button {
                            refreshSSHHosts()
                        } label: {
                            if isRefreshingSSH {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Image(systemName: "arrow.clockwise")
                            }
                        }
                        .buttonStyle(.borderless)
                        .disabled(isRefreshingSSH)
                        .help("Reload Hosts from ~/.ssh/config")
                        .accessibilityLabel("Refresh SSH hosts")
                    }
                } footer: {
                    Text("Loaded from ~/.ssh/config. Enable a host, then Connect. For OTP/captcha, run `ssh <alias>` in Terminal first so ControlMaster is ready.")
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
            .onAppear {
                if settings.machines.isEmpty {
                    refreshSSHHosts()
                }
            }
        }
    }

    private func refreshSSHHosts() {
        isRefreshingSSH = true
        Task { @MainActor in
            tunnels.refreshFromLocalSSH()
            isRefreshingSSH = false
        }
    }

    @ViewBuilder
    private func machineRow(_ machine: MachineConfig) -> some View {
        let state = tunnels.state(for: machine.id)
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(machine.alias.isEmpty ? "Untitled" : machine.alias)
                        .font(.body.weight(.semibold))
                    Text(machine.user.isEmpty
                         ? machine.hostName
                         : "\(machine.user)@\(machine.hostName)")
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
                Spacer()
                machineStateBadge(state)
            }

            if let err = tunnels.error(for: machine.id), state == .error {
                Text(err)
                    .font(.caption)
                    .foregroundStyle(Color(nsColor: .systemRed))
                    .lineLimit(4)
            }

            HStack(spacing: 12) {
                Toggle("Enabled", isOn: Binding(
                    get: { machine.enabled },
                    set: { on in
                        var m = machine
                        m.enabled = on
                        settings.upsertMachine(m)
                        tunnels.syncConfig()
                        if on {
                            tunnels.connect(id: m.id)
                        } else {
                            tunnels.disconnect(id: m.id, clearError: true)
                        }
                    }
                ))
                .toggleStyle(.switch)
                .labelsHidden()
                Text(machine.enabled ? "On" : "Off")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Spacer()

                PanelIconButton(
                    systemName: "bolt.horizontal",
                    help: "Connect",
                    accessibilityLabel: "Connect",
                    enabled: machine.enabled && state != .connecting
                ) {
                    tunnels.connect(id: machine.id)
                }
                PanelIconButton(
                    systemName: "xmark",
                    help: "Disconnect",
                    accessibilityLabel: "Disconnect",
                    enabled: state != .idle
                ) {
                    tunnels.disconnect(id: machine.id, clearError: true)
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func machineStateBadge(_ state: MachineLinkState) -> some View {
        let (label, color): (String, NSColor) = {
            switch state {
            case .idle: return ("Idle", .secondaryLabelColor)
            case .connecting: return ("Connecting", .systemOrange)
            case .connected: return ("Connected", .systemGreen)
            case .error: return ("Error", .systemRed)
            }
        }()
        return Text(label)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Color(nsColor: color).opacity(0.15), in: Capsule())
            .foregroundStyle(Color(nsColor: color))
    }

    // MARK: Appearance

    private var appearancePane: some View {
        SettingsPage(tab: .appearance) {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Length")
                            Spacer()
                            Text(ribbonLengthLabel)
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        Slider(
                            value: Binding(
                                get: { settings.ribbonLengthScale },
                                set: {
                                    settings.ribbonLengthScale = $0
                                    onRibbonRefresh?()
                                }
                            ),
                            in: SettingsStore.ribbonLengthScaleRange,
                            step: 0.25
                        )
                        .accessibilityLabel("Ribbon length")
                        .accessibilityValue(ribbonLengthLabel)

                        Text("How far the ribbon stretches as more work is active.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Thickness")
                            Spacer()
                            Text(ribbonThicknessLabel)
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        Slider(
                            value: Binding(
                                get: { settings.ribbonThickness },
                                set: {
                                    settings.ribbonThickness = $0
                                    onRibbonRefresh?()
                                }
                            ),
                            in: SettingsStore.ribbonThicknessRange,
                            step: 0.5
                        )
                        .accessibilityLabel("Ribbon thickness")
                        .accessibilityValue(ribbonThicknessLabel)

                        Text("Height of the band inside the menu-bar item.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    Button {
                        settings.ribbonLengthScale = SettingsStore.defaultRibbonLengthScale
                        settings.ribbonThickness = SettingsStore.defaultRibbonThickness
                        onRibbonRefresh?()
                    } label: {
                        Label("Reset Size", systemImage: "arrow.counterclockwise")
                    }
                    .disabled(
                        settings.ribbonLengthScale == SettingsStore.defaultRibbonLengthScale
                            && settings.ribbonThickness == SettingsStore.defaultRibbonThickness
                    )
                } header: {
                    Text("Ribbon Size")
                } footer: {
                    Text("Length grows with active work; thickness stays fixed. Applies immediately.")
                }

                Section {
                    ForEach(Status.painted, id: \.self) { status in
                        ColorPicker(
                            selection: Binding(
                                get: { settings.statusColors.color(for: status).color },
                                set: { newColor in
                                    var map = settings.statusColors
                                    map.set(RGBColor(newColor), for: status)
                                    settings.statusColors = map
                                    onRibbonRefresh?()
                                }
                            ),
                            supportsOpacity: false
                        ) {
                            HStack(spacing: 10) {
                                Image(systemName: statusSymbol(status))
                                    .symbolRenderingMode(.hierarchical)
                                    .foregroundStyle(settings.statusColors.color(for: status).color)
                                    .frame(width: 18)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(status.title)
                                    Text(statusColorHint(status))
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }

                    Button {
                        settings.resetStatusColors()
                        onRibbonRefresh?()
                    } label: {
                        Label("Restore Defaults", systemImage: "arrow.counterclockwise")
                    }
                    .disabled(settings.statusColors.isDefault)
                } header: {
                    Text("Status Colors")
                } footer: {
                    Text("Shared by the menu-bar ribbon and status panel. Light/dark follows macOS.")
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
        }
    }

    private var ribbonLengthLabel: String {
        let pct = Int((settings.ribbonLengthScale * 100).rounded())
        return "\(pct)%"
    }

    private var ribbonThicknessLabel: String {
        let t = settings.ribbonThickness
        if t == t.rounded() {
            return "\(Int(t)) pt"
        }
        return String(format: "%.1f pt", t)
    }

    // MARK: Notifications

    private var notifyPolicyBinding: Binding<String> {
        Binding(
            get: { store.notify.policy == .all ? "all" : "single" },
            set: { store.notifyPolicySink?($0) }
        )
    }

    private var notificationsPane: some View {
        SettingsPage(tab: .notifications) {
            Form {
                Section {
                    Picker("When several surfaces are open", selection: notifyPolicyBinding) {
                        Text("One surface (recommended)").tag("single")
                        Text("Every surface").tag("all")
                    }
                    .help("The hub elects one owner when macOS Nerve and VS Code are both connected, so the same Ask is not bannered twice.")
                    if let owner = store.notify.owner, store.notify.policy == .single {
                        Text("Current owner: \(owner)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                } header: {
                    Text("Broadcast")
                } footer: {
                    Text("One surface lets the hub pick a single notifier from the connected Nerve surfaces. Every surface is the old behaviour: each app that has notifications on may fire.")
                }

                Section {
                    PreferenceToggleRow(
                        title: "Pause all",
                        description: "Silence every Nerve notification until you turn this off.",
                        isOn: $settings.notificationsPaused
                    )

                    PreferenceToggleRow(
                        title: "Play sounds",
                        description: "Use the system sound for allowed alerts.",
                        isOn: $settings.notificationSoundEnabled
                    )
                } header: {
                    Text("Delivery")
                }

                Section {
                    NotificationToggle(title: "Your turn", systemImage: "hand.raised", isOn: $settings.notifyRequired)
                    NotificationToggle(title: "Urgent ask", systemImage: "bell", isOn: $settings.notifyUrgent)
                    NotificationToggle(title: "Failures", systemImage: "xmark.circle", isOn: $settings.notifyFailure)
                    NotificationToggle(title: "Unresponsive", systemImage: "bolt.slash", isOn: $settings.notifyUnresponsive)
                    NotificationToggle(title: "Long task done", systemImage: "checkmark.circle", isOn: $settings.notifyLongSuccess)
                } header: {
                    Text("Events")
                } footer: {
                    Text("Your turn gently reminds when an agent needs input, review, or a decision (Ask reasons at suggested+). System waits only colour the ribbon — they do not interrupt. Suggested Ask stays silent even when Play sounds is on. Failures cover outcome.failure and tool/turn failures. Never free-text.")
                }

                Section {
                    PreferenceToggleRow(
                        title: "Quiet hours",
                        description: "Hold notifications during a daily window.",
                        isOn: $settings.dndEnabled
                    )

                    if settings.dndEnabled {
                        HStack(spacing: 18) {
                            DatePicker("From", selection: timeBinding(\.dndStartMinutes), displayedComponents: .hourAndMinute)
                            DatePicker("Until", selection: timeBinding(\.dndEndMinutes), displayedComponents: .hourAndMinute)
                        }
                        .datePickerStyle(.field)
                    }
                } header: {
                    Text("Schedule")
                }

                if !store.knownProducers.isEmpty {
                    Section {
                        ForEach(store.knownProducers, id: \.id) { source in
                            Toggle(source.name ?? source.id, isOn: Binding(
                                get: { settings.mutedSourceIds.contains(source.id) },
                                set: { _ in settings.toggleMute(producerId: source.id) }
                            ))
                            .toggleStyle(.switch)
                        }
                    } header: {
                        Text("Muted Producers")
                    } footer: {
                        Text("Producers that have reported at least once.")
                    }
                }

                if !store.knownProjects.isEmpty {
                    Section {
                        ForEach(store.knownProjects, id: \.self) { project in
                            Toggle(project, isOn: Binding(
                                get: { settings.mutedProjectIds.contains(project) },
                                set: { _ in settings.toggleMuteProject(project) }
                            ))
                            .toggleStyle(.switch)
                        }
                    } header: {
                        Text("Muted Projects")
                    }
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
        }
    }

    // MARK: About

    private var aboutPane: some View {
        SettingsPage(tab: .about) {
            ScrollView {
                VStack(spacing: 20) {
                    VStack(spacing: 12) {
                        Image("Logo")
                            .resizable()
                            .interpolation(.high)
                            .aspectRatio(contentMode: .fit)
                            .frame(width: 88, height: 88)
                            .shadow(color: Color.accentColor.opacity(0.16), radius: 12, y: 4)
                            .accessibilityHidden(true)

                        VStack(spacing: 3) {
                            Text("Nerve")
                                .font(.title2.weight(.semibold))
                            Text("Version \(appVersion)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }

                        Text("A quiet menu-bar pulse for agents, builds, and long-running local work.")
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 390)
                    }

                    VStack(spacing: 0) {
                        AboutInfoRow(
                            title: "Private by design",
                            detail: "Jobs, timelines, and pending actions stay in memory. Only preferences are saved on this Mac.",
                            systemImage: "lock.shield.fill",
                            tint: Color(nsColor: .systemGreen)
                        )

                        Divider()
                            .padding(.leading, 52)

                        AboutInfoRow(
                            title: "Local endpoint",
                            detail: "\(NerveEndpoint.baseURL.absoluteString)/v1/snapshot",
                            systemImage: "network",
                            tint: Color(nsColor: .systemBlue),
                            monospacedDetail: true
                        )
                    }
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
                    }

                    Button("Show Welcome Tips…") {
                        FirstRunCoachController().show { loadDemo in
                            if loadDemo {
                                store.loadDemo()
                                onRibbonRefresh?()
                            }
                            settings.hasSeenCoachMarks = true
                        }
                    }
                    .buttonStyle(.bordered)
                }
                .padding(.horizontal, 28)
                .padding(.top, 12)
                .padding(.bottom, 28)
                .frame(maxWidth: 500)
                .frame(maxWidth: .infinity)
            }
        }
    }

    private func timeBinding(_ keyPath: ReferenceWritableKeyPath<SettingsStore, Int>) -> Binding<Date> {
        Binding(
            get: {
                let minutes = settings[keyPath: keyPath]
                return Calendar.current.date(
                    bySettingHour: minutes / 60,
                    minute: minutes % 60,
                    second: 0,
                    of: Date()
                ) ?? Date()
            },
            set: { date in
                let components = Calendar.current.dateComponents([.hour, .minute], from: date)
                settings[keyPath: keyPath] = (components.hour ?? 0) * 60 + (components.minute ?? 0)
            }
        )
    }

    private func statusColorHint(_ status: Status) -> String {
        switch status {
        case .problem: return "Failed or cannot continue"
        case .attention: return "Needs your input, approval, or review"
        case .waiting: return "Waiting on the system; no action needed"
        case .running: return "Actively working"
        case .monitor: return "Watching a background stream"
        case .success: return "Turn complete, or finished successfully"
        case .inactive: return "Ready, paused, or unknown"
        }
    }

    private func statusSymbol(_ status: Status) -> String {
        switch status {
        case .problem: return "xmark.circle.fill"
        case .attention: return "exclamationmark.circle.fill"
        case .waiting: return "clock"
        case .running: return "play.circle.fill"
        case .monitor: return "dot.radiowaves.left.and.right"
        case .success: return "checkmark.circle.fill"
        case .inactive: return "pause.circle.fill"
        }
    }
}

// MARK: - Settings scene fallback

struct SettingsView: View {
    var body: some View {
        ContentUnavailableView {
            Label("Nerve Settings", systemImage: "gearshape")
        } description: {
            Text("Open Settings from the menu-bar ribbon.")
        }
        .frame(width: 340, height: 180)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}
