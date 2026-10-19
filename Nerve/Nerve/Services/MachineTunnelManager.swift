import Foundation
import Observation

/// A cache and command adapter for hub-owned machine connections.
@MainActor
@Observable
final class MachineTunnelManager {
    private(set) var linkState: [UUID: MachineLinkState] = [:]
    private(set) var lastError: [UUID: String] = [:]
    private weak var settings: SettingsStore?
    private var poll: Task<Void, Never>?
    func attach(settings: SettingsStore) { self.settings = settings }
    func start() {
        guard poll == nil else { return }
        let legacy = settings?.machines ?? []
        poll = Task { [weak self] in
            if let self {
                for machine in legacy {
                    await self.send("v1/machines",method:"PUT",body:["id":machine.id.uuidString,"host":machine.alias,"enabled":machine.enabled,"autoConnect":machine.autoConnect])
                }
            }
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }
    // Stopping this UI never stops SSH connections owned by the hub.
    func stopAll() { poll?.cancel(); poll = nil }
    func state(for id: UUID) -> MachineLinkState { linkState[id] ?? .idle }
    func error(for id: UUID) -> String? { lastError[id] }
    func refreshFromLocalSSH() {
        Task { await send("v1/machines/discover", method:"POST"); await refresh() }
    }
    func syncConfig() {
        guard let settings else { return }
        let machines = settings.machines
        Task {
            for machine in machines {
                await send("v1/machines",method:"PUT",body:["id":machine.id.uuidString,"host":machine.alias,
                    "enabled":machine.enabled,"autoConnect":machine.autoConnect])
            }
            await refresh()
        }
    }
    func connect(id: UUID) {
        Task {
            if let machine = settings?.machine(id:id) {
                await send("v1/machines",method:"PUT",body:["id":machine.id.uuidString,"host":machine.alias,
                    "enabled":machine.enabled,"autoConnect":machine.autoConnect])
            }
            await send("v1/machines/\(id.uuidString)/connect",method:"POST")
            await refresh()
        }
    }
    func disconnect(id: UUID, clearError: Bool = false) {
        Task { await send("v1/machines/\(id.uuidString)/disconnect",method:"POST"); await refresh() }
    }
    private func send(_ path: String, method: String, body:[String:Any]? = nil) async {
        var request = URLRequest(url:NerveEndpoint.baseURL.appending(path:path))
        request.httpMethod = method
        request.timeoutInterval = 5
        if let body { request.setValue("application/json",forHTTPHeaderField:"Content-Type"); request.httpBody = try? JSONSerialization.data(withJSONObject:body) }
        _ = try? await URLSession.shared.data(for:request)
    }
    private func refresh() async {
        do {
            let (data,response) = try await URLSession.shared.data(from:NerveEndpoint.baseURL.appending(path:"v1/machines"))
            guard (response as? HTTPURLResponse)?.statusCode == 200,
                  let rows = try JSONSerialization.jsonObject(with:data) as? [[String:Any]] else { return }
            var machines:[MachineConfig] = []
            for row in rows {
                guard let rawId = row["id"] as? String, let id = UUID(uuidString:rawId), let host = row["host"] as? String else { continue }
                machines.append(MachineConfig(id:id,alias:host,hostName:row["hostName"] as? String ?? host,
                    user:row["user"] as? String ?? "",enabled:row["enabled"] as? Bool ?? false,autoConnect:row["autoConnect"] as? Bool ?? false))
                switch row["state"] as? String {
                case "connected": linkState[id] = .connected
                case "connecting": linkState[id] = .connecting
                case "offline": linkState[id] = .error
                default: linkState[id] = .idle
                }
                lastError[id] = row["error"] as? String
            }
            settings?.machines = machines
        } catch { }
    }
}
