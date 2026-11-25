import Foundation

@main
enum EQPresetTests {
    @MainActor
    static func main() {
        let defaults = UserDefaults(suiteName: "EQPresetTests-\(UUID().uuidString)")!
        let store = EQPresetStore(defaults: defaults)

        // Names are trimmed and capped at 13 characters.
        assert(EQPreset.cleanName("  Late night  ") == "Late night")
        assert(EQPreset.cleanName("Fourteen chars").count == EQPreset.maxNameLength)

        // Built-ins are matched and highlighted by value.
        assert(store.preset(matching: [0, 0, 0])?.name == "Flat")
        assert(store.preset(matching: [8, 4, 2]) == nil)
        assert(store.preset(matching: []) == nil)

        // Saving, duplicate names, and blank names.
        let mine = store.save(name: "Mine", values: [8, 4, 2])
        assert(mine != nil)
        assert(store.preset(matching: [8, 4, 2])?.id == mine?.id)
        assert(store.save(name: "mine", values: [1, 1, 1]) == nil)
        assert(store.save(name: "flat", values: [1, 1, 1]) == nil)
        assert(store.save(name: "   ", values: [1, 1, 1]) == nil)

        // Renaming keeps the cap, allows the preset's own name, and rejects others'.
        store.rename(mine!.id, to: "Commute mix for trains")
        assert(store.custom.first?.name == "Commute mix f")
        assert(store.isNameAvailable("commute mix f", excluding: mine!.id))
        store.rename(mine!.id, to: "Bass")
        assert(store.custom.first?.name == "Commute mix f")

        // Custom presets persist across launches; deleting removes them.
        assert(EQPresetStore(defaults: defaults).custom.map(\.name) == ["Commute mix f"])
        store.delete(mine!.id)
        assert(EQPresetStore(defaults: defaults).custom.isEmpty)

        print("EQ preset tests passed")
    }
}
