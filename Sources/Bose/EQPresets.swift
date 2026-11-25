import Foundation

/// A named Bass/Mid/Treble setting. Built-ins ship with the app; custom ones
/// are saved by the user and persisted in UserDefaults.
struct EQPreset: Identifiable, Codable, Equatable {
    var id: String
    var name: String
    /// Band values ordered by EQ band ID (Bass, Mid, Treble).
    var values: [Int]
    var isBuiltIn = false
    /// SF Symbol shown on the chip. Built-ins have one; custom presets fall
    /// back to a star.
    var icon: String?

    var symbol: String { icon ?? "star.fill" }

    static let maxNameLength = 13

    static func cleanName(_ raw: String) -> String {
        String(raw.trimmingCharacters(in: .whitespacesAndNewlines).prefix(maxNameLength))
    }

    static let builtIns: [EQPreset] = [
        EQPreset(id: "builtin.flat", name: "Flat", values: [0, 0, 0], isBuiltIn: true, icon: "waveform"),
        EQPreset(id: "builtin.bass", name: "Bass", values: [6, 0, 0], isBuiltIn: true, icon: "speaker.wave.3.fill"),
        EQPreset(id: "builtin.vocal", name: "Vocal", values: [-2, 4, 2], isBuiltIn: true, icon: "mic.fill"),
        EQPreset(id: "builtin.warm", name: "Warm", values: [4, 0, -3], isBuiltIn: true, icon: "flame.fill"),
        EQPreset(id: "builtin.bright", name: "Bright", values: [0, 0, 5], isBuiltIn: true, icon: "sun.max.fill"),
        EQPreset(id: "builtin.night", name: "Night", values: [-3, 2, -2], isBuiltIn: true, icon: "moon.fill")
    ]
}

@MainActor
final class EQPresetStore: ObservableObject {
    @Published private(set) var custom: [EQPreset] = []

    private let defaults: UserDefaults
    private static let key = "customEQPresets"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: Self.key),
           let saved = try? JSONDecoder().decode([EQPreset].self, from: data) {
            custom = saved
        }
    }

    var all: [EQPreset] { EQPreset.builtIns + custom }

    /// The first preset whose values equal the headphones' current EQ.
    func preset(matching values: [Int]) -> EQPreset? {
        guard !values.isEmpty else { return nil }
        return all.first { $0.values == values }
    }

    /// Whether `name` is free to use, ignoring the preset being renamed.
    func isNameAvailable(_ name: String, excluding id: String? = nil) -> Bool {
        let clean = EQPreset.cleanName(name)
        guard !clean.isEmpty else { return false }
        return !all.contains {
            $0.id != id && $0.name.compare(clean, options: .caseInsensitive) == .orderedSame
        }
    }

    @discardableResult
    func save(name: String, values: [Int]) -> EQPreset? {
        guard isNameAvailable(name) else { return nil }
        let preset = EQPreset(id: UUID().uuidString, name: EQPreset.cleanName(name), values: values)
        custom.append(preset)
        persist()
        return preset
    }

    func rename(_ id: String, to name: String) {
        guard isNameAvailable(name, excluding: id),
              let index = custom.firstIndex(where: { $0.id == id }) else { return }
        custom[index].name = EQPreset.cleanName(name)
        persist()
    }

    func delete(_ id: String) {
        custom.removeAll { $0.id == id }
        persist()
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(custom) {
            defaults.set(data, forKey: Self.key)
        }
    }
}
