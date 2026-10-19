import Foundation
import Observation

/// Executes the hub's bootstrap command; never owns the daemon.
@MainActor @Observable
final class HubProcessManager {
    private static let binaryName = "nerve-hub"
    enum LaunchState: Equatable { case idle, running, notInstalled, unreachable(String) }
    private(set) var state: LaunchState = .idle
    private var ensuring = false
    func ensureRunning() async {
        guard !ensuring else { return }
        ensuring = true
        defer { ensuring = false }
        guard let binary=Self.locateBinary() else { state = .notInstalled; return }
        let result: (Int32,String) = await Task.detached {
            let process=Process()
            let errors=Pipe()
            process.executableURL=binary
            process.arguments=["ensure"]
            process.standardInput=FileHandle.nullDevice
            process.standardOutput=FileHandle.nullDevice
            process.standardError=errors
            do {
                try process.run()
                let data=errors.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                return (process.terminationStatus,String(data:data,encoding:.utf8) ?? "Hub unavailable")
            } catch { return (1,error.localizedDescription) }
        }.value
        state = result.0 == 0 ? .running : .unreachable(result.1)
    }
    static func locateBinary() -> URL? {
        let fileManager = FileManager.default
        for directory in searchDirectories() {
            let candidate = directory.appending(path: binaryName)
            if fileManager.isExecutableFile(atPath: candidate.path) {
                return candidate
            }
        }
        return nil
    }

    private static func searchDirectories() -> [URL] {
        var directories: [URL] = []

        // `Contents/MacOS` — where `./scripts/nerve.sh --app` embeds it.
        if let executables = Bundle.main.executableURL?.deletingLastPathComponent() {
            directories.append(executables)
        }
        // `Contents/Resources` — a packaging path that treats the hub as a
        // resource rather than an auxiliary executable still works.
        if let resources = Bundle.main.resourceURL {
            directories.append(resources)
        }

        directories.append(URL(fileURLWithPath: "/opt/homebrew/bin"))
        directories.append(URL(fileURLWithPath: "/usr/local/bin"))
        directories.append(
            FileManager.default.homeDirectoryForCurrentUser.appending(path: ".cargo/bin")
        )

        let path = ProcessInfo.processInfo.environment["PATH"] ?? ""
        for segment in path.split(separator: ":") where !segment.isEmpty {
            directories.append(URL(fileURLWithPath: String(segment)))
        }

        return directories
    }

}
