import Foundation

struct Headphones: Identifiable, Equatable {
    let address: String
    var name: String
    var connected: Bool
    var id: String { address }
}

struct BosePeer: Identifiable, Equatable {
    let mac: String
    var name: String
    var connected: Bool
    var isLocal: Bool
    var isBoseProduct: Bool
    var active: Bool = false
    var id: String { mac }

    var displayName: String {
        if name.isEmpty || name == mac { return mac }
        return name
    }

    var symbolName: String {
        if isLocal { return "laptopcomputer" }
        let lower = name.lowercased()
        if lower.contains("ipad") || lower.contains("pad") { return "ipad" }
        if lower.contains("iphone") || lower.contains("phone") { return "iphone" }
        if lower.contains("macbook") || lower.contains("mac") { return "laptopcomputer" }
        if lower.contains("windows") || lower.contains("pc") { return "pc" }
        if isBoseProduct { return "headphones" }
        return "pc"
    }

    var statusText: String {
        if isLocal {
            return active ? "This Mac · Audio" : "This Mac"
        }
        if connected {
            return active ? "Connected · Audio" : "Connected"
        }
        return "Not connected"
    }
}

struct BoseMode: Identifiable, Equatable {
    let index: Int
    var name: String
    var rawCNC: Int
    var id: Int { index }
}

struct EQBand: Identifiable, Equatable {
    let bandID: Int
    var name: String
    var minimum: Int
    var maximum: Int
    var value: Int
    var id: Int { bandID }
}

struct PeerSwitch: Identifiable, Equatable {
    let target: BosePeer
    let candidates: [BosePeer]
    var id: String { target.mac + "|" + candidates.map(\.mac).joined(separator: ",") }
}

enum SourceKind: Int {
    case none = 0
    case bluetooth = 1
    case auxiliary = 2
}
