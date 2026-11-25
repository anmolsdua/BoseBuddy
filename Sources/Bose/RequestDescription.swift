import Foundation

/// Plain-English log text for BMAP requests, with the raw codes kept in
/// brackets for debugging, e.g.
/// "Headphones don't support reading charging state [2.3 get, error 4 unsupported function]".
enum RequestDescription {
    /// "reading charging state", "changing EQ", "requesting mode list".
    static func action(fblock: UInt8, function: UInt8, op: BmapOperator) -> String {
        let feature = features["\(fblock).\(function)"] ?? "function \(fblock).\(function)"
        switch op {
        case .get: return "reading \(feature)"
        case .set, .setGet: return "changing \(feature)"
        default: return "requesting \(feature)"
        }
    }

    /// The headphones answered with an error packet.
    static func deviceError(fblock: UInt8, function: UInt8, op: BmapOperator, code: UInt8) -> String {
        let action = action(fblock: fblock, function: function, op: op)
        let sentence: String
        switch code {
        case 3, 4: sentence = "Headphones don't support \(action)"
        case 5: sentence = "Headphones don't allow \(action)"
        case 6: sentence = "Headphones rejected the value when \(action)"
        case 7: sentence = "Headphones have no data for \(action)"
        case 8, 12: sentence = "Headphones were busy and couldn't finish \(action)"
        case 1: sentence = "Headphones rejected the message length when \(action)"
        default: sentence = "Headphones returned an error when \(action)"
        }
        return sentence + " [\(fblock).\(function) \(op), error \(code) \(BmapError.name(code))]"
    }

    /// The request itself failed: no reply, not connected, or a transport error.
    static func failure(fblock: UInt8, function: UInt8, op: BmapOperator,
                        error: Error, seconds: TimeInterval) -> String {
        let action = action(fblock: fblock, function: function, op: op)
        let codes = " [\(fblock).\(function) \(op)]"
        switch error {
        case BmapError.timeout:
            return "No reply from the headphones when \(action) after "
                + String(format: "%.1fs", seconds) + codes
        case BmapError.notConnected:
            return "Not connected when \(action)" + codes
        default:
            return "Failed \(action): \(error)" + codes
        }
    }

    private static let features: [String: String] = [
        "0.5": "firmware version",
        "1.2": "headphone name",
        "1.3": "voice prompts",
        "1.5": "noise cancelling level",
        "1.7": "EQ",
        "1.9": "button settings",
        "1.10": "multipoint",
        "1.11": "sidetone",
        "2.2": "battery level",
        "2.3": "charging state",
        "4.1": "device connection",
        "4.2": "device disconnection",
        "4.4": "paired device list",
        "4.5": "device info",
        "4.8": "pairing mode",
        "5.1": "audio source",
        "7.4": "power",
        "31.1": "mode list",
        "31.3": "current mode",
        "31.6": "mode settings"
    ]
}
