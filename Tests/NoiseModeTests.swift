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
        let outdoor = record("03 00 08 01 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 03 00 00 00 00")
        let empty = record("03 00 00 01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00")

        let quietMode = BoseMode(record: quiet)
        assert(quietMode?.index == 0 && quietMode?.name == "Quiet" && quietMode?.rawCNC == 0)
        assert(BoseMode(record: quiet)?.ancLabel == "ANC 10/10")
        let awareMode = BoseMode(record: aware)
        assert(awareMode?.index == 1 && awareMode?.name == "Aware" && awareMode?.rawCNC == 10)
        assert(BoseMode(record: aware)?.ancLabel == "ANC 0/10")

        // Unnamed ready-made mode: named from its template ID, Wind Block on.
        let walkMode = BoseMode(record: walk)
        assert(walkMode?.name == "Walk")
        assert(walkMode?.windBlock == true)
        assert(walkMode?.ancLabel == "ANC Auto · Wind Block")

        assert(walkMode?.editable == true)
        assert(BoseMode(record: quiet)?.editable == false)

        // Wind Block is byte 46, not the capability bits in byte 41, which
        // Outdoor shares with Walk.
        let outdoorMode = BoseMode(record: outdoor)
        assert(outdoorMode?.name == "Outdoor")
        assert(outdoorMode?.windBlock == false)
        assert(outdoorMode?.ancLabel == "ANC 7/10")

        // Unused slots are skipped and reported as free; used ones aren't.
        assert(BoseMode(record: empty) == nil)
        assert(BoseMode.isFreeSlot(record: empty))
        assert(!BoseMode.isFreeSlot(record: outdoor))
        assert(!BoseMode.isFreeSlot(record: quiet))
        var unknown = walk
        unknown[2] = 0x7E
        assert(BoseMode(record: unknown)?.name == "Mode 3")

        // Our Custom mode keeps its name; once the Bose app reuses the slot
        // (prompt 0x07 Commute, old name bytes left behind) the prompt wins.
        var ours = record("03 00 00 01 01 01 43 75 73 74 6f 6d 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 00 00 00 00 00")
        assert(BoseMode(record: ours)?.name == "Custom")
        let takenOver = record("03 00 07 01 01 01 43 75 73 74 6f 6d 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 09 05 00 00 00 00")
        assert(BoseMode(record: takenOver)?.name == "Commute")
        assert(BoseMode(record: takenOver)?.ancLevel == 5)
        ours[6] = UInt8(ascii: "M")  // a name the user chose is kept
        assert(BoseMode(record: ours)?.name == "Mustom")

        // Writes are 39 bytes: index, prompt, name, CNC, auto, spatial, wind.
        let write = outdoorMode!.configPayload(rawCNC: 5, windBlock: false)
        assert(write.count == 39)
        assert(Array(write.prefix(3)) == [3, 0x00, 0x08])
        assert(write[3..<35].allSatisfy { $0 == 0 })
        assert(Array(write.suffix(4)) == [5, 0, 0, 0])
        assert(walkMode!.configPayload(rawCNC: 3, windBlock: true).suffix(4) == [3, 0, 0, 1])
        assert(walkMode!.configPayload(rawCNC: 42, windBlock: false)[35] == 10)

        let custom = BoseMode.custom(slot: 3, rawCNC: 4).configPayload(rawCNC: 4, windBlock: false)
        assert(Array(custom.prefix(3)) == [3, 0, 0])
        assert(ByteCodec.string(custom[3..<35]) == "Custom")
        assert(Array(custom.suffix(4)) == [4, 0, 0, 0])

        // Short or 0xFF records are rejected.
        assert(BoseMode(record: Array(quiet.prefix(20))) == nil)
        var none = quiet
        none[0] = 0xFF
        assert(BoseMode(record: none) == nil)

        print("Noise mode tests passed")
    }
}
