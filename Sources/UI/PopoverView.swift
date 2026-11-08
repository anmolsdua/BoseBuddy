import AppKit
import SwiftUI

private struct RenderModeKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    var boseBarRenderMode: Bool {
        get { self[RenderModeKey.self] }
        set { self[RenderModeKey.self] = newValue }
    }
}

enum Theme {
    static let accent = Color(red: 0.15, green: 0.40, blue: 0.95)
}

struct PopoverView: View {
    @ObservedObject var model: BoseController
    @Environment(\.boseBarRenderMode) private var renderMode

    @State private var showEqualizer = false
    @State private var confirmPowerOff = false

    private var isConnected: Bool { model.linkState.isConnected }

    private var currentModeName: String? {
        guard let index = model.modeIndex else { return nil }
        return model.modes.first(where: { $0.index == index })?.name
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            if let error = model.errorText {
                errorBanner(error)
            }
            if model.pairingMode {
                pairingBanner
            }
            Divider().opacity(0.6)
            if renderMode {
                VStack(spacing: 12) { content }
                    .padding(14)
                    .frame(width: 360)
            } else {
                ScrollView(.vertical, showsIndicators: false) {
                    VStack(spacing: 12) { content }
                        .padding(14)
                }
            }
            Divider().opacity(0.6)
            footer
        }
        .frame(width: 360, height: renderMode ? 780 : 562)
        .background(Color(nsColor: .windowBackgroundColor))
        .tint(Theme.accent)
        .alert("Power off headphones?", isPresented: $confirmPowerOff) {
            Button("Cancel", role: .cancel) {}
            Button("Power Off", role: .destructive) {
                Task { await model.powerOff() }
            }
        } message: {
            Text("The headphones will turn off and disconnect from every device.")
        }
    }

    @ViewBuilder
    private var content: some View {
        if isConnected {
            devicesCard
            modesCard
            if model.supportsEQ || model.supportsSidetone {
                equalizerCard
            }
            Text(model.firmware.isEmpty ? " " : "Firmware \(model.firmware)")
                .font(.system(size: 9))
                .foregroundStyle(.tertiary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.top, 2)
        } else {
            connectState
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(Theme.accent.opacity(isConnected ? 0.14 : 0.07))
                Image(systemName: "headphones")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundStyle(isConnected ? Theme.accent : Color.secondary)
            }
            .frame(width: 44, height: 44)

            VStack(alignment: .leading, spacing: 3) {
                Text(model.deviceName)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.middle)

                HStack(spacing: 5) {
                    Circle()
                        .fill(statusColor)
                        .frame(width: 6, height: 6)
                    Text(headerStatusText)
                        .font(.system(size: 9.5))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .minimumScaleFactor(0.8)
                }
            }

            Spacer(minLength: 4)

            if isConnected, let battery = model.battery {
                batteryPill(battery)
            } else if !isConnected {
                Button {
                    Task { await model.connect() }
                } label: {
                    Text(model.linkState.isConnecting ? "Connecting…" : "Connect")
                        .font(.system(size: 12, weight: .medium))
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.accent)
                .controlSize(.small)
                .disabled(model.linkState.isConnecting)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }

    private var headerStatusText: String {
        switch model.linkState {
        case .connected:
            let names = connectionNames
            return "Connected to: " + (names.isEmpty ? "none" : names.joined(separator: " + "))
        case .connecting:
            return "Connecting…"
        case .failed, .disconnected:
            return "Not connected"
        }
    }

    private var connectionNames: [String] {
        var names: [String] = []
        if model.linkState.isConnected { names.append("Mac") }
        for peer in model.peers where peer.connected && !peer.isLocal {
            let name = shortAudioName(peer.displayName)
            if !names.contains(name) { names.append(name) }
        }
        return names
    }

    private func shortAudioName(_ name: String) -> String {
        let lower = name.lowercased()
        if lower.contains("iphone") { return "iPhone" }
        if lower.contains("ipad") || lower.contains("pad") { return "iPad" }
        if lower.contains("macbook") || lower.contains("mac") { return "Mac" }
        return name
    }

    private var statusColor: Color {
        switch model.linkState {
        case .connected: return .green
        case .connecting: return .orange
        case .failed: return .red
        case .disconnected: return Color.secondary.opacity(0.5)
        }
    }

    private func batteryPill(_ battery: Int) -> some View {
        HStack(spacing: 5) {
            Image(systemName: model.charging ? "bolt.fill" : batterySymbol(battery))
                .font(.system(size: 10, weight: .bold))
            Text("\(battery)%")
                .font(.system(size: 12, weight: .semibold, design: .rounded))
                .monospacedDigit()
        }
        .foregroundStyle(batteryColor(battery))
        .padding(.horizontal, 9)
        .padding(.vertical, 5)
        .background(Capsule().fill(batteryColor(battery).opacity(0.12)))
    }

    private func batterySymbol(_ level: Int) -> String {
        switch level {
        case 0...10: return "battery.0"
        case 11...25: return "battery.25"
        case 26...50: return "battery.50"
        case 51...75: return "battery.75"
        default: return "battery.100"
        }
    }

    private func batteryColor(_ level: Int) -> Color {
        if level <= 15 { return .red }
        if level <= 30 { return .orange }
        return .green
    }

    // MARK: - Banners

    private func errorBanner(_ text: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 11))
                .foregroundStyle(.orange)
            Text(text)
                .font(.system(size: 11))
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 2)
            Button {
                model.errorText = nil
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color.orange.opacity(0.10))
    }

    private var pairingBanner: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.system(size: 12))
                .foregroundStyle(.blue)
            VStack(alignment: .leading, spacing: 2) {
                Text("Pairing mode is on")
                    .font(.system(size: 11, weight: .semibold))
                Text("Select the headphones on the device you want to add.")
                    .font(.system(size: 10))
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 2)
            Button {
                model.clearPairingMode()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color.blue.opacity(0.10))
    }

    // MARK: - Devices

    private var devicesCard: some View {
        Card {
            HStack(spacing: 6) {
                SectionTitle("Devices")
                audioSlotBadge
                Spacer()
                if model.supportsMultipoint {
                    Text("Multipoint")
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                    MiniSwitch(isOn: model.multipoint) {
                        Task { await model.setMultipoint(!model.multipoint) }
                    }
                    .help("Multipoint keeps the headphones connected to two audio devices at once")
                }
            }

            if let note = model.actionNote {
                HStack(spacing: 7) {
                    ProgressView().controlSize(.mini)
                    Text(note)
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            if let pending = model.pendingSwitch {
                pendingSwitchCard(pending)
            }

            if model.peers.isEmpty {
                HStack(spacing: 8) {
                    if !model.peersLoaded {
                        ProgressView().controlSize(.mini)
                    }
                    Text(!model.peersLoaded
                         ? "Reading paired devices…"
                         : model.supportsPeers
                            ? "No devices paired with the headphones."
                            : "Couldn't read the headphones' device list.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, 4)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(model.peers.enumerated()), id: \.element.id) { index, peer in
                        if index > 0 {
                            Divider().opacity(0.4).padding(.leading, 28)
                        }
                        peerRow(peer)
                    }
                }
            }

            if connectedPeerCount >= 2 {
                Text(model.confirmBeforeSwitch
                     ? "Both audio slots are in use. Switching asks which device to disconnect."
                     : "Both audio slots are in use. Switching disconnects the non-Mac device automatically.")
                    .font(.system(size: 10))
                    .foregroundStyle(.tertiary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var audioSlotBadge: some View {
        Text("\(connectedPeerCount)/2 audio")
            .font(.system(size: 9, weight: .medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(Capsule().fill(Color.primary.opacity(0.06)))
    }

    private var connectedPeerCount: Int {
        model.peers.filter { $0.connected || ($0.isLocal && model.audioAvailable) }.count
    }

    private func pendingSwitchCard(_ pending: PeerSwitch) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 6) {
                Image(systemName: "arrow.triangle.2.circlepath")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.orange)
                Text("Both audio slots are in use")
                    .font(.system(size: 11, weight: .semibold))
            }
            Text("Disconnect which device to connect \(pending.target.displayName)?")
                .font(.system(size: 10))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            ForEach(pending.candidates) { candidate in
                Button {
                    model.confirmPendingSwitch(dropping: candidate)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: candidate.symbolName)
                            .font(.system(size: 11))
                            .frame(width: 16)
                        Text(candidate.displayName)
                            .font(.system(size: 11, weight: .medium))
                            .lineLimit(1)
                        Spacer()
                        Text("Disconnect")
                            .font(.system(size: 10))
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 5)
                    .padding(.horizontal, 7)
                    .background(
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(Color.primary.opacity(0.05))
                    )
                }
                .buttonStyle(.plain)
            }

            Button("Cancel") { model.cancelPendingSwitch() }
                .buttonStyle(.borderless)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
        }
        .padding(9)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.orange.opacity(0.10))
        )
    }

    private func peerRow(_ peer: BosePeer) -> some View {
        HStack(spacing: 9) {
            Image(systemName: peer.symbolName)
                .font(.system(size: 13))
                .frame(width: 18)
                .foregroundStyle(isAudioConnected(peer) ? Color.primary : Color.secondary)

            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 4) {
                    if isAudioConnected(peer) {
                        Circle()
                            .fill(Color.green)
                            .frame(width: 6, height: 6)
                    }
                    Text(peer.displayName)
                        .font(.system(size: 12, weight: peer.isLocal ? .semibold : .regular))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    if peer.active {
                        Image(systemName: "speaker.wave.2.fill")
                            .font(.system(size: 8))
                            .foregroundStyle(Theme.accent)
                    }
                }
                Text(peerStatus(peer))
                    .font(.system(size: 10))
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 4)

            if model.peerAction == peer.mac || (peer.isLocal && model.busyAction == "audio") {
                ProgressView().controlSize(.mini)
            } else if peer.isLocal {
                if model.audioAvailable {
                    Button("Disconnect") {
                        Task { await model.disconnectPeer(peer) }
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .disabled(model.busyAction != nil)
                } else {
                    Button("Connect audio") {
                        Task { await model.connectPeer(peer) }
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(Theme.accent)
                    .disabled(model.busyAction != nil)
                    .help("Bring this Mac online as an audio device")
                }
            } else if peer.connected {
                Button("Disconnect") {
                    Task { await model.disconnectPeer(peer) }
                }
                .buttonStyle(.borderless)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .disabled(model.peerAction != nil)
            } else if model.supportsMultipoint && !model.multipoint {
                Text("Multipoint off")
                    .font(.system(size: 10))
                    .foregroundStyle(.tertiary)
            } else {
                Button(blockingPeer(for: peer) == nil ? "Connect" : "Switch") {
                    Task { await model.connectPeer(peer) }
                }
                .buttonStyle(.borderless)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(Theme.accent)
                .disabled(model.peerAction != nil)
                .help(blockingPeer(for: peer).map {
                    "Connect \(peer.displayName) and disconnect \($0.displayName)"
                } ?? "Connect \(peer.displayName)")
            }
        }
        .padding(.vertical, 6)
    }

    private func blockingPeer(for peer: BosePeer) -> BosePeer? {
        guard connectedPeerCount >= 2 else { return nil }
        return model.peers.first { $0.connected && !$0.isLocal && $0.mac != peer.mac }
    }

    private func isAudioConnected(_ peer: BosePeer) -> Bool {
        peer.isLocal ? model.linkState.isConnected : peer.connected
    }

    private func peerStatus(_ peer: BosePeer) -> String {
        if peer.isLocal {
            return model.audioAvailable ? "This Mac · Control + Audio" : "This Mac · Control"
        }
        if peer.connected {
            return peer.active ? "Connected · Audio" : "Connected"
        }
        return "Not connected"
    }

    private var activeOtherPeer: BosePeer? {
        guard let mac = model.activePeerMAC else { return nil }
        return model.peers.first(where: { $0.mac == mac && !$0.isLocal })
    }

    // MARK: - Modes

    private var modesCard: some View {
        Card {
            HStack {
                SectionTitle("Noise control")
                Spacer()
                if let mode = model.modes.first(where: { $0.index == model.modeIndex }) {
                    Text("ANC \(10 - min(10, max(0, mode.rawCNC)))/10")
                        .font(.system(size: 10, weight: .medium, design: .rounded))
                        .foregroundStyle(.tertiary)
                }
            }

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 74), spacing: 6)], spacing: 6) {
                ForEach(model.modes) { mode in
                    modeChip(mode)
                }
            }

            if model.busyAction == "mode" {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.mini)
                    Text("Switching…").font(.system(size: 10)).foregroundStyle(.secondary)
                }
            }
        }
    }

    private func modeChip(_ mode: BoseMode) -> some View {
        let selected = model.modeIndex == mode.index
        return Button {
            Task { await model.setMode(mode.index) }
        } label: {
            Text(mode.name)
                .font(.system(size: 11, weight: .medium))
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 7)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(selected ? Theme.accent : Color.primary.opacity(0.06))
                )
                .foregroundStyle(selected ? Color.white : Color.primary)
        }
        .buttonStyle(.plain)
        .disabled(model.busyAction != nil)
    }

    // MARK: - Equalizer

    private var equalizerCard: some View {
        Card {
            DisclosureGroup(isExpanded: $showEqualizer) {
                VStack(spacing: 10) {
                    ForEach(model.eqBands) { band in
                        EQRow(band: band) { value in
                            Task { await model.setEQ(band: band, value: value) }
                        }
                    }
                    if model.supportsSidetone {
                        Divider().padding(.vertical, 2)
                        HStack {
                            Text("Sidetone")
                                .font(.system(size: 11))
                            Spacer()
                            Picker("", selection: Binding(
                                get: { model.sidetone },
                                set: { level in Task { await model.setSidetone(level) } }
                            )) {
                                Text("Off").tag(0)
                                Text("High").tag(1)
                                Text("Med").tag(2)
                                Text("Low").tag(3)
                            }
                            .labelsHidden()
                            .pickerStyle(.segmented)
                            .controlSize(.small)
                            .frame(width: 168)
                        }
                    }
                }
                .padding(.top, 10)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "slider.horizontal.3")
                        .font(.system(size: 11))
                    Text("Equalizer")
                        .font(.system(size: 12, weight: .medium))
                    Spacer()
                    if model.busyAction?.hasPrefix("eq") == true {
                        ProgressView().controlSize(.mini)
                    }
                }
                .foregroundStyle(.primary)
            }
            .tint(.secondary)
        }
    }

    // MARK: - Empty state

    private var connectState: some View {
        VStack(spacing: 12) {
            Image(systemName: "headphones")
                .font(.system(size: 30, weight: .light))
                .foregroundStyle(.secondary)
                .padding(.top, 24)

            Text("Connect your headphones")
                .font(.system(size: 14, weight: .semibold))

            Text("Connect the headphones from the side button or the Bose app on your phone, then click Connect.")
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 290)

            Button {
                model.refreshPairedDevices()
                Task { await model.connect() }
            } label: {
                Text(model.linkState.isConnecting ? "Connecting…" : "Connect")
                    .frame(minWidth: 90)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.accent)
            .controlSize(.regular)
            .disabled(model.linkState.isConnecting)

            Text("Use the headphones' Bluetooth button, or the Bose app on your iPhone/iPad/Android, to connect this Mac.")
                .font(.system(size: 10))
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 290)

        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
    }

    // MARK: - Footer

    private var footer: some View {
        HStack(spacing: 8) {
            if isConnected {
                Button {
                    Task { await model.enterPairingMode() }
                } label: {
                    Label("Pair new device", systemImage: "plus")
                        .font(.system(size: 11, weight: .medium))
                }
                .buttonStyle(.borderless)
                .disabled(model.busyAction != nil)
            }

            Spacer()

            Button {
                Task { await model.refresh(deep: true) }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 11, weight: .semibold))
            }
            .buttonStyle(.borderless)
            .help("Refresh")
            .disabled(!isConnected)

            Menu {
                Button("Reconnect") {
                    model.disconnect()
                    Task { await model.connect() }
                }
                Divider()
                Toggle("Show battery in menu bar", isOn: $model.showBatteryInMenuBar)
                Toggle("Ask which device to disconnect", isOn: $model.confirmBeforeSwitch)
                Toggle("Launch at login", isOn: Binding(
                    get: { model.launchAtLogin },
                    set: { model.setLaunchAtLogin($0) }
                ))
                Divider()
                Button("Setup Guide…") {
                    NotificationCenter.default.post(name: .showSetupGuide, object: nil)
                }
                Button("Open Bluetooth Settings") { model.openBluetoothSettings() }
                Button("Power off headphones") { confirmPowerOff = true }
                    .disabled(!isConnected)
                Divider()
                Button("Quit BoseBuddy") { NSApp.terminate(nil) }
            } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 11, weight: .semibold))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
    }
}

// MARK: - Building blocks

private struct Card<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            content
        }
        .padding(11)
        .background(
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .fill(Color.primary.opacity(0.035))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.06), lineWidth: 0.5)
        )
    }
}

private struct MiniSwitch: View {
    let isOn: Bool
    let action: () -> Void

    var body: some View {
        ZStack(alignment: isOn ? .trailing : .leading) {
            Capsule()
                .fill(isOn ? Theme.accent : Color.primary.opacity(0.18))
                .frame(width: 32, height: 19)
            Circle()
                .fill(.white)
                .shadow(color: .black.opacity(0.15), radius: 1, y: 0.5)
                .frame(width: 15, height: 15)
                .padding(2)
        }
        .contentShape(Rectangle())
        .onTapGesture(perform: action)
        .animation(.easeInOut(duration: 0.15), value: isOn)
        .accessibilityAddTraits(.isButton)
        .accessibilityValue(isOn ? "On" : "Off")
    }
}

private struct SectionTitle: View {
    let text: String
    init(_ text: String) { self.text = text }

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.secondary)
            .textCase(.uppercase)
            .kerning(0.4)
    }
}

private struct EQRow: View {
    let band: EQBand
    let onCommit: (Int) -> Void

    @State private var draft: Double

    init(band: EQBand, onCommit: @escaping (Int) -> Void) {
        self.band = band
        self.onCommit = onCommit
        _draft = State(initialValue: Double(band.value))
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(band.name)
                .font(.system(size: 11))
                .frame(width: 44, alignment: .leading)
            Slider(
                value: $draft,
                in: Double(band.minimum)...Double(band.maximum),
                step: 1,
                onEditingChanged: { editing in
                    if !editing { onCommit(Int(draft)) }
                }
            )
            .controlSize(.mini)
            Text(draft > 0 ? "+\(Int(draft))" : "\(Int(draft))")
                .font(.system(size: 10, weight: .medium, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 24, alignment: .trailing)
        }
        .onChange(of: band.value) { _, newValue in
            draft = Double(newValue)
        }
    }
}
