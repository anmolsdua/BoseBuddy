import Foundation

@main
enum NoiseModeTests {
    static func record(_ hex: String) -> [UInt8] {
        hex.split(separator: " ").map { UInt8($0, radix: 16)! }
    }

    static func main() {
        // Mode records captured from a QC45 (firmware 4.0.4).
        let quiet = record("00 00 01 00 00 01 51 75 69 65 74 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00")
        let aware = record("01 00 02 00 00 01 41 77 61 72 65 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 0a 00 00 00 00")
        let walk = record("02 00 15 01 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 01")
        let empty = record("03 00 00 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00")

        assert(BoseMode(record: quiet) == BoseMode(index: 0, name: "Quiet", rawCNC: 0))
        assert(BoseMode(record: quiet)?.ancLabel == "ANC 10/10")
        assert(BoseMode(record: aware) == BoseMode(index: 1, name: "Aware", rawCNC: 10))
        assert(BoseMode(record: aware)?.ancLabel == "ANC 0/10")

        // Unnamed ready-made mode: named from its template ID, Wind Block on.
        let walkMode = BoseMode(record: walk)
        assert(walkMode?.name == "Walk")
        assert(walkMode?.windBlock == true)
        assert(walkMode?.ancLabel == "ANC Auto · Wind Block")

        // Unused slots and unknown unnamed templates are skipped.
        assert(BoseMode(record: empty) == nil)
        var unknown = walk
        unknown[2] = 0x7E
        assert(BoseMode(record: unknown) == nil)

        // Short or 0xFF records are rejected.
        assert(BoseMode(record: Array(quiet.prefix(20))) == nil)
        var none = quiet
        none[0] = 0xFF
        assert(BoseMode(record: none) == nil)

        print("Noise mode tests passed")
    }
}
