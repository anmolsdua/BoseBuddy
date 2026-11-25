import Foundation

/// A noise-control mode as stored on the headphones (AudioModes, fblock 31).
struct BoseMode: Identifiable, Equatable {
    let index: Int
    var name: String
    /// 0 is full noise cancelling, 10 is fully aware.
    var rawCNC: Int
    /// Wind Block adjusts noise cancelling automatically, so the mode has no
    /// fixed ANC level.
    var windBlock = false
    var id: Int { index }

    /// Used when the headphones don't report their modes.
    static let fallback = [
        BoseMode(index: 0, name: "Quiet", rawCNC: 0),
        BoseMode(index: 1, name: "Aware", rawCNC: 10)
    ]

    /// Text for the Noise control header.
    var ancLabel: String {
        windBlock ? "ANC Auto · Wind Block" : "ANC \(10 - min(10, max(0, rawCNC)))/10"
    }

    /// Parses one mode record (31.6). Returns nil for empty slots and for
    /// unnamed modes this app can't name.
    init?(record bytes: [UInt8]) {
        guard bytes.count >= 38 else { return nil }
        let index = Int(bytes[0])
        guard index != 0xFF else { return nil }

        // Bose's ready-made modes (e.g. Walk) carry a template ID in byte 2
        // instead of a name; byte 5 marks the slot as in use.
        let name = ByteCodec.string(bytes[6..<38])
        let inUse = bytes[5] != 0
        let templateName = Self.templateNames[bytes[2]]
        if name.isEmpty && index > 1 && (!inUse || templateName == nil) { return nil }

        let cncOffset = bytes.count >= 48 ? 38 : 42
        // In the 47-byte layout the byte before the CNC level holds flags;
        // 0x08 is Wind Block (the QC45's Walk mode sets it).
        self.init(
            index: index,
            name: name.isEmpty ? (templateName ?? Self.defaultName(index)) : name,
            rawCNC: bytes.count > cncOffset ? Int(bytes[cncOffset]) : 0,
            windBlock: cncOffset == 42 && (bytes[41] & 0x08) != 0
        )
    }

    init(index: Int, name: String, rawCNC: Int, windBlock: Bool = false) {
        self.index = index
        self.name = name
        self.rawCNC = rawCNC
        self.windBlock = windBlock
    }

    /// Names for Bose's ready-made modes, keyed by the template ID the
    /// headphones report. Seen on a QC45 (firmware 4.0.4).
    private static let templateNames: [UInt8: String] = [
        0x15: "Walk"
    ]

    private static func defaultName(_ index: Int) -> String {
        switch index {
        case 0: return "Quiet"
        case 1: return "Aware"
        case 2: return "Immersion"
        case 3: return "Cinema"
        default: return "Mode \(index + 1)"
        }
    }
}
