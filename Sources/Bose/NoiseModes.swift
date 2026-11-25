import Foundation

/// A noise-control mode as stored on the headphones (AudioModes, fblock 31).
///
/// ModeConfig [31.6] STATUS layout (47 bytes, QC45 / QC Headphones):
///   [0] index, [1-2] voice prompt ID, [3] editable, [4] configured,
///   [5] slot in use, [6-37] name, [41] capability bits, [42] CNC level,
///   [43] auto CNC, [44] spatial, [46] wind block.
/// SETGET takes 39 bytes: index, prompt (2), name (32), CNC, auto CNC,
/// spatial, wind block. Layout from bosectl (github.com/aaronsb/bosectl),
/// confirmed on a QC45 running firmware 4.0.4.
struct BoseMode: Identifiable, Equatable {
    let index: Int
    var name: String
    /// 0 is full noise cancelling, 10 is fully aware.
    var rawCNC: Int
    /// Wind Block adjusts noise cancelling automatically; while it is on the
    /// headphones store CNC 0 regardless of what is written.
    var windBlock = false
    /// User modes (slots 2-3 on a QC45) can be rewritten; Quiet/Aware can't.
    var editable = false
    var prompt: [UInt8] = [0, 0]
    /// Stored name bytes; empty for Bose's ready-made modes, which are named
    /// by their prompt.
    var storedName = ""
    var autoCNC = false
    var spatial: UInt8 = 0
    var id: Int { index }

    /// Used when the headphones don't report their modes.
    static let fallback = [
        BoseMode(index: 0, name: "Quiet", rawCNC: 0),
        BoseMode(index: 1, name: "Aware", rawCNC: 10)
    ]

    static let customName = "Custom"

    /// Noise cancelling strength shown to the user: 10 is full ANC.
    var ancLevel: Int { 10 - min(10, max(0, rawCNC)) }

    /// Text for the Noise control header.
    var ancLabel: String {
        windBlock ? "ANC Auto · Wind Block" : "ANC \(ancLevel)/10"
    }

    init(index: Int, name: String, rawCNC: Int, windBlock: Bool = false) {
        self.index = index
        self.name = name
        self.rawCNC = rawCNC
        self.windBlock = windBlock
    }

    /// Parses one mode record (31.6). Returns nil for unused slots.
    init?(record bytes: [UInt8]) {
        guard bytes.count >= 38 else { return nil }
        let index = Int(bytes[0])
        guard index != 0xFF else { return nil }
        let storedName = ByteCodec.string(bytes[6..<38])
        let inUse = bytes[5] != 0
        if storedName.isEmpty && index > 1 && !inUse { return nil }

        let full = bytes.count >= 47
        let cncOffset = bytes.count >= 48 ? 38 : 42
        let promptName = Self.promptNames[bytes[2]]
        // Our Custom mode has no prompt. When the Bose app reuses its slot it
        // writes a template prompt but can leave our name bytes behind, so a
        // prompt wins over a leftover "Custom".
        let takenOver = storedName == Self.customName && promptName != nil
        self.init(
            index: index,
            name: storedName.isEmpty || takenOver ? (promptName ?? Self.defaultName(index)) : storedName,
            rawCNC: bytes.count > cncOffset ? Int(bytes[cncOffset]) : 0,
            windBlock: full && bytes.count < 48 && bytes[46] != 0
        )
        self.editable = bytes[3] != 0
        self.prompt = [bytes[1], bytes[2]]
        self.storedName = storedName
        if full && bytes.count < 48 {
            self.autoCNC = bytes[43] != 0
            self.spatial = bytes[44]
        }
    }

    /// Whether a record describes an editable slot with no mode in it.
    static func isFreeSlot(record bytes: [UInt8]) -> Bool {
        guard bytes.count >= 38, bytes[0] != 0xFF else { return false }
        return bytes[3] != 0 && bytes[5] == 0 && ByteCodec.string(bytes[6..<38]).isEmpty
    }

    /// 39-byte ModeConfig SETGET payload that rewrites this mode, keeping its
    /// prompt, name, auto CNC and spatial settings.
    func configPayload(rawCNC: Int, windBlock: Bool) -> [UInt8] {
        var nameBytes = [UInt8](storedName.utf8.prefix(31))
        nameBytes += [UInt8](repeating: 0, count: 32 - nameBytes.count)
        return [UInt8(index)] + prompt + nameBytes + [
            UInt8(min(10, max(0, rawCNC))),
            autoCNC ? 1 : 0,
            spatial,
            windBlock ? 1 : 0
        ]
    }

    /// A new user mode named "Custom" for a free slot.
    static func custom(slot: Int, rawCNC: Int) -> BoseMode {
        var mode = BoseMode(index: slot, name: customName, rawCNC: rawCNC)
        mode.editable = true
        mode.storedName = customName
        return mode
    }

    /// Bose's mode voice prompts, keyed by the prompt ID's low byte. The
    /// headphones store no name for ready-made modes such as Walk (0x15).
    private static let promptNames: [UInt8: String] = [
        0x01: "Quiet", 0x02: "Aware", 0x03: "Transparent", 0x04: "Transparency",
        0x05: "Masking", 0x06: "Comfort", 0x07: "Commute", 0x08: "Outdoor",
        0x09: "Workout", 0x0A: "Home", 0x0B: "Work", 0x0C: "Music",
        0x0D: "Focus", 0x0E: "Relax", 0x0F: "Flight", 0x10: "Airport",
        0x11: "Driving", 0x12: "Training", 0x13: "Gym", 0x14: "Run",
        0x15: "Walk", 0x16: "Hike", 0x17: "Talk", 0x18: "Call",
        0x19: "Whisper", 0x1A: "Hearing", 0x1B: "Learn", 0x1C: "Podcast",
        0x1D: "Audiobook", 0x1E: "Calm", 0x1F: "Sleep", 0x20: "Meditate",
        0x21: "Yoga", 0x22: "Immersion", 0x23: "Stereo", 0x24: "Cinema"
    ]

    private static func defaultName(_ index: Int) -> String {
        switch index {
        case 0: return "Quiet"
        case 1: return "Aware"
        default: return "Mode \(index + 1)"
        }
    }
}
