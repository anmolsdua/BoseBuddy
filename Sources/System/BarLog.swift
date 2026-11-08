import Foundation

/// Tiny append-only log at ~/Library/Logs/BoseBuddy.log. Useful for diagnosing
/// Bluetooth/audio state changes without attaching a debugger.
enum BarLog {
    private static let queue = DispatchQueue(label: "com.bosebuddy.app.log")
    private static let maxSize = 512 * 1024

    static var url: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Logs", isDirectory: true)
            .appendingPathComponent("BoseBuddy.log")
    }

    static func write(_ message: String) {
        queue.async {
            let line = "\(ISO8601DateFormatter().string(from: Date())) \(message)\n"
            guard let data = line.data(using: .utf8) else { return }
            let url = Self.url
            do {
                try FileManager.default.createDirectory(at: url.deletingLastPathComponent(),
                                                        withIntermediateDirectories: true)
                if let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
                   let size = attributes[.size] as? Int, size > Self.maxSize {
                    try? FileManager.default.removeItem(at: url)
                }
                if FileManager.default.fileExists(atPath: url.path) {
                    let handle = try FileHandle(forWritingTo: url)
                    try handle.seekToEnd()
                    try handle.write(contentsOf: data)
                    try handle.close()
                } else {
                    try data.write(to: url)
                }
            } catch {
                // Logging must never affect the app.
            }
        }
    }
}
