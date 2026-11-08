import Foundation

/// BMAP operator (low nibble of the flags byte).
enum BmapOperator: UInt8 {
    case set = 0
    case get = 1
    case setGet = 2
    case status = 3
    case error = 4
    case start = 5
    case result = 6
    case processing = 7
}

/// One BMAP packet: [fblock, function, flags, length, payload...].
struct BmapPacket {
    var fblock: UInt8
    var function: UInt8
    var deviceID: UInt8 = 0
    var port: UInt8 = 0
    var op: BmapOperator
    var payload: Data

    init(fblock: UInt8, function: UInt8, deviceID: UInt8 = 0, port: UInt8 = 0,
         op: BmapOperator, payload: Data = Data()) {
        self.fblock = fblock
        self.function = function
        self.deviceID = deviceID
        self.port = port
        self.op = op
        self.payload = payload
    }

    init(fblock: UInt8, function: UInt8, op: BmapOperator, payload: [UInt8],
         port: UInt8 = 0) {
        self.init(fblock: fblock, function: function, port: port, op: op,
                  payload: Data(payload))
    }

    func encoded() -> Data {
        let body = payload.prefix(255)
        let flags = (deviceID << 6) | (port << 4) | (op.rawValue & 0x0F)
        var out = Data([fblock, function, flags, UInt8(body.count)])
        out.append(body)
        return out
    }

    /// Decodes every complete packet in `data`. Partial trailing packets are ignored.
    static func parseAll(_ data: Data) -> [BmapPacket] {
        let bytes = [UInt8](data)
        var packets: [BmapPacket] = []
        var i = 0
        while i + 4 <= bytes.count {
            let length = Int(bytes[i + 3])
            let end = i + 4 + length
            guard end <= bytes.count else { break }
            let flags = bytes[i + 2]
            packets.append(BmapPacket(
                fblock: bytes[i],
                function: bytes[i + 1],
                deviceID: (flags >> 6) & 0x03,
                port: (flags >> 4) & 0x03,
                op: BmapOperator(rawValue: flags & 0x0F) ?? .status,
                payload: Data(bytes[(i + 4)..<end])
            ))
            i = end
        }
        return packets
    }
}

enum BmapError: LocalizedError {
    case notConnected
    case noResponse
    case timeout
    case transport(String)
    case device(code: UInt8, fblock: UInt8, function: UInt8)

    var errorDescription: String? {
        switch self {
        case .notConnected:
            return "Not connected to the headphones."
        case .noResponse:
            return "The headphones didn't return any data."
        case .timeout:
            return "The headphones didn't respond in time."
        case .transport(let message):
            return message
        case .device(let code, let fblock, let function):
            return "Headphones returned \(BmapError.name(code)) for [\(fblock).\(function)]."
        }
    }

    static func name(_ code: UInt8) -> String {
        switch code {
        case 0: return "unknown error"
        case 1: return "bad length"
        case 2: return "checksum error"
        case 3: return "unsupported block"
        case 4: return "unsupported function"
        case 5: return "operator not supported"
        case 6: return "invalid data"
        case 7: return "data unavailable"
        case 8: return "temporary failure"
        case 9: return "timeout"
        case 10: return "invalid state"
        case 11: return "device not found"
        case 12: return "device busy"
        case 15: return "invalid transition"
        case 20: return "insecure transport"
        default: return "error \(code)"
        }
    }
}

enum ByteCodec {
    static func macBytes(_ mac: String) -> [UInt8] {
        mac.split(whereSeparator: { $0 == ":" || $0 == "-" })
            .compactMap { UInt8($0, radix: 16) }
    }

    /// Canonical display form, e.g. BC:87:FA:5B:FB:D7.
    static func formatMAC(_ bytes: [UInt8]) -> String {
        bytes.map { String(format: "%02X", $0) }.joined(separator: ":")
    }

    static func canonicalMAC(_ raw: String?) -> String {
        guard let raw else { return "" }
        let bytes = macBytes(raw)
        return bytes.count == 6 ? formatMAC(bytes) : raw.uppercased()
    }

    /// Returns a printable UTF-8 string, stopping at the first NUL.
    static func string(_ bytes: ArraySlice<UInt8>) -> String {
        var slice = bytes.prefix { $0 != 0 }
        while let first = slice.first, first < 0x20 { slice = slice.dropFirst() }
        while let last = slice.last, last < 0x20 { slice = slice.dropLast() }
        return String(decoding: slice, as: UTF8.self)
    }
}
