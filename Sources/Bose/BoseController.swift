import AppKit
import Combine
import Foundation
import IOBluetooth
import ServiceManagement

@MainActor
final class BoseController: ObservableObject {
    enum LinkState: Equatable {
        case disconnected
        case connecting
        case connected
        case failed(String)

        var isConnected: Bool {
            if case .connected = self { return true }
            return false
        }

        var isConnecting: Bool {
            if case .connecting = self { return true }
            return false
        }
    }

    // MARK: - Published state

    @Published private(set) var linkState: LinkState = .disconnected
    @Published private(set) var headphones: [Headphones] = []
    @Published private(set) var deviceName = "Bose headphones"
    @Published private(set) var productName = ""
    @Published private(set) var firmware = ""
    @Published private(set) var battery: Int?
    @Published private(set) var charging = false

    @Published private(set) var modes: [BoseMode] = []
    @Published private(set) var modeIndex: Int?
    @Published private(set) var eqBands: [EQBand] = []
    @Published private(set) var multipoint = false
    @Published private(set) var sidetone = 0

    @Published private(set) var peers: [BosePeer] = []
    @Published private(set) var activePeerMAC: String?
    @Published private(set) var sourceKind: SourceKind = .none
    @Published private(set) var pairingMode = false
    @Published private(set) var audioAvailable = false

    @Published private(set) var supportsModes = false
    @Published private(set) var supportsEQ = false
    @Published private(set) var supportsMultipoint = false
    @Published private(set) var supportsSidetone = false
    @Published private(set) var supportsPeers = false
    @Published private(set) var peersLoaded = false
    @Published private(set) var channelID: UInt8 = 0

    @Published var busyAction: String?
    @Published var peerAction: String?
    @Published private(set) var actionNote: String?
    @Published private(set) var pendingSwitch: PeerSwitch?
    @Published var errorText: String?
    @Published var showBatteryInMenuBar: Bool {
        didSet {
            UserDefaults.standard.set(showBatteryInMenuBar, forKey: Self.batteryMenuBarKey)
            onStatusChange?()
        }
    }
    @Published var launchAtLogin = false
    @Published var confirmBeforeSwitch: Bool {
        didSet {
            UserDefaults.standard.set(confirmBeforeSwitch, forKey: Self.confirmSwitchKey)
            if !confirmBeforeSwitch {
                pendingSwitch = nil
            }
        }
    }

    var onStatusChange: (() -> Void)?

    // MARK: - Private state

    private static let batteryMenuBarKey = "showBatteryInMenuBar"
    private static let confirmSwitchKey = "confirmBeforeSwitch"
    private let audioMonitor = AudioOutputMonitor()
    private var link: RFCOMMLink?
    private var isRefreshing = false
    private var pollTimer: Timer?
    private let watchdogQueue = DispatchQueue(label: "com.bosebuddy.app.watchdog")
    private var watchdogSource: DispatchSourceTimer?
    private var connectedAddress: String?
    private var lightPollCount = 0
    private var hasProbedSidetone = false

    init() {
        showBatteryInMenuBar = UserDefaults.standard.object(forKey: Self.batteryMenuBarKey) as? Bool ?? true
        confirmBeforeSwitch = UserDefaults.standard.object(forKey: Self.confirmSwitchKey) as? Bool ?? false
        launchAtLogin = SMAppService.mainApp.status == .enabled

        audioMonitor.onChange = { [weak self] available in
            Task { @MainActor in
                guard let self else { return }
                if self.audioAvailable != available {
                    self.audioAvailable = available
                    BarLog.write("macOS audio available: \(available) [listener]")
                    self.onStatusChange?()
                }
            }
        }
        audioMonitor.nameHint = "bose"
        audioAvailable = audioMonitor.refresh()
    }

    // MARK: - Lifecycle

    func start() {
        Task {
            let found = await Task.detached(priority: .utility) { Self.pairedHeadphones() }.value
            headphones = found
            // Never initiate a new Bluetooth connection from here. Only attach
            // if the Mac is already connected, so opening BoseBuddy cannot
            // displace another device's connection.
            if let connected = found.first(where: { $0.connected }) {
                await connect(address: connected.address)
            } else if audioAvailable, let first = found.first {
                await connect(address: first.address)
            }
        }
    }

    func popoverOpened() {
        refreshAudioAvailability()
        refreshPairedDevicesInBackground()
        if linkState.isConnected {
            Task {
                await readBattery()
                await readSource()
                await loadPeers()
            }
        } else if linkState != .connecting,
                  headphones.contains(where: { $0.connected }) || (audioAvailable && !headphones.isEmpty) {
            Task { await connect() }
        }
        startPolling()
    }

    func popoverClosed() {
        pollTimer?.invalidate()
        pollTimer = nil
        pendingSwitch = nil
    }

    /// IOBluetooth's pairedDevices can block on the Bluetooth TCC prompt, so it
    /// never runs on the main thread at launch.
    nonisolated private static func pairedHeadphones() -> [Headphones] {
        let devices = (IOBluetoothDevice.pairedDevices() as? [IOBluetoothDevice]) ?? []
        let found: [Headphones] = devices.compactMap { device -> Headphones? in
            guard let rawName = device.name, Self.looksLikeBose(rawName) else { return nil }
            return Headphones(
                address: ByteCodec.canonicalMAC(device.addressString),
                name: Self.friendlyName(rawName),
                connected: device.isConnected()
            )
        }
        return found.sorted { lhs, rhs in
            if lhs.connected != rhs.connected { return lhs.connected && !rhs.connected }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
    }

    private func applyPairedDevices(_ found: [Headphones]) {
        headphones = found
        if linkState.isConnected == false, let first = found.first {
            if case .failed = linkState {} else { deviceName = first.name }
        }
    }

    func refreshPairedDevices() {
        applyPairedDevices(Self.pairedHeadphones())
    }

    func refreshPairedDevicesInBackground() {
        Task.detached(priority: .utility) {
            let found = Self.pairedHeadphones()
            await MainActor.run { self.applyPairedDevices(found) }
        }
    }

    // MARK: - Connection

    func connect(address: String? = nil) async {
        guard !linkState.isConnected, !linkState.isConnecting else { return }
        linkState = .connecting
        errorText = nil
        pairingMode = false

        if headphones.isEmpty {
            let found = await Task.detached(priority: .utility) { Self.pairedHeadphones() }.value
            headphones = found
        }

        let target = address
            ?? headphones.first(where: { $0.connected })?.address
            ?? (audioAvailable ? headphones.first?.address : nil)

        guard let target else {
            let message = "Connect the headphones from the side button or the Bose app on your phone, then try again."
            linkState = .failed(message)
            errorText = message
            return
        }

        await openClassic(address: target)
    }

    private func openClassic(address: String) async {
        let pairedName = headphones.first(where: { $0.address == address })?.name ?? ""
        if !pairedName.isEmpty { deviceName = Self.friendlyName(pairedName) }
        let newLink = RFCOMMLink()
        do {
            let channel = try await newLink.open(
                address: address,
                channels: Self.channelCandidates(for: pairedName),
                timeout: 8,
                allowExistingAudio: audioAvailable
            )
            newLink.onClosed = { [weak self] in
                Task { @MainActor in self?.handleControlClosed() }
            }
            link = newLink
            channelID = channel
            connectedAddress = address
            linkState = .connected
            BarLog.write("Bluetooth connection established on channel \(channel) to \(address)")
            startWatchdog()
            onStatusChange?()

            if let name = try? await readProductName(), !name.isEmpty {
                let clean = Self.friendlyName(name)
                productName = clean
                deviceName = clean
                audioMonitor.nameHint = clean
            }
            firmware = (try? await readFirmware()) ?? ""
            await refresh(deep: true)
        } catch {
            newLink.close()
            link = nil
                let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            BarLog.write("connect failed: \(message)")
            linkState = .failed(message)
            errorText = message
        }
        onStatusChange?()
    }

    func disconnect() {
        pollTimer?.invalidate()
        pollTimer = nil
        stopWatchdog()
        connectedAddress = nil
        linkState = .disconnected
        link?.close()
        link = nil
        battery = nil
        charging = false
        modes = []
        modeIndex = nil
        eqBands = []
        peers = []
        activePeerMAC = nil
        multipoint = false
        pairingMode = false
        pendingSwitch = nil
        supportsPeers = false
        peersLoaded = false
        onStatusChange?()
    }

    // MARK: - Refresh

    func refresh(deep: Bool) async {
        guard linkState.isConnected, !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        await readBattery()
        await readSource()
        await readModeIndex()

        guard deep else {
            lightPollCount += 1
            if lightPollCount % 4 == 0 { await loadPeers() }
            return
        }

        await loadMultipoint()
        await loadSidetone()
        await loadPeers()
        await loadModes()
        await loadEQ()
    }

    private func startPolling() {
        pollTimer?.invalidate()
        pollTimer = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                await self.refresh(deep: false)
            }
        }
    }

    // MARK: - Reads

    private func readProductName() async throws -> String {
        let packet = try await get(1, 2)
        return ByteCodec.string([UInt8](packet.payload)[...])
    }

    private func readFirmware() async throws -> String {
        let packet = try await get(0, 5)
        return String(decoding: packet.payload, as: UTF8.self)
            .trimmingCharacters(in: .controlCharacters)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func readBattery() async {
        guard linkState.isConnected else { return }
        do {
            let packet = try await get(2, 2)
            if let first = packet.payload.first {
                battery = Int(first)
            }
            if let packet = try? await get(2, 3), let first = packet.payload.first {
                charging = first != 0
            }
        } catch {
            if case BmapError.notConnected = error {
                markDisconnected()
                return
            }
        }
        onStatusChange?()
    }

    private func markDisconnected() {
        handleControlClosed()
    }

    private func handleControlClosed() {
        guard linkState.isConnected else { return }
        BarLog.write("Bluetooth connection closed")
        link = nil
        linkState = .disconnected
        channelID = 0
        stopWatchdog()
        errorText = "Bluetooth connection lost. Mac audio may be unaffected; tap Connect to reconnect."
        onStatusChange?()
    }

    func refreshAudioAvailability() {
        audioMonitor.nameHint = deviceName.isEmpty ? "bose" : deviceName
        let available = audioMonitor.refresh()
        if available != audioAvailable {
            audioAvailable = available
            BarLog.write("macOS audio available: \(available)")
            NSLog("BoseBuddy: audio available %@", available ? "yes" : "no")
            onStatusChange?()
        }
    }

    private func startWatchdog() {
        stopWatchdog()
        BarLog.write("watchdog started")
        let source = DispatchSource.makeTimerSource(queue: watchdogQueue)
        source.schedule(deadline: .now() + 2, repeating: 2)
        source.setEventHandler { [weak self] in
            guard let self else { return }
            // CoreAudio enumeration is safe off the main thread. The hint is
            // intentionally generic; Bose product names all contain "bose".
            let available = AudioOutputMonitor.boseOutputExists(hint: "bose")
            Task { @MainActor in
                self.applyWatchdog(audio: available)
            }
        }
        source.resume()
        watchdogSource = source
    }

    private func stopWatchdog() {
        watchdogSource?.cancel()
        watchdogSource = nil
    }

    private func applyWatchdog(audio: Bool) {
        guard linkState.isConnected else {
            stopWatchdog()
            return
        }
        if audio != audioAvailable {
            audioAvailable = audio
            BarLog.write("macOS audio available: \(audio) [watchdog]")
            onStatusChange?()
        }
    }


    /// Ask the headphones to bring this Mac online as an audio device. This is
    /// the same CONNECT command used for other peers, targeted at the local MAC.
    /// Without it macOS only gets an SPP link, so no audio output appears.
    func connectMacAudio(automatic: Bool = false) async {
        guard linkState.isConnected else {
            if !automatic { errorText = "Connect the headphones first." }
            return
        }
        if audioAvailable { return }
        if !automatic { busyAction = "audio" }
        defer { if !automatic { busyAction = nil } }

        refreshAudioAvailability()
        if audioAvailable { return }
        if peers.isEmpty { await loadPeers() }
        guard let local = peers.first(where: { $0.isLocal }) else {
            if !automatic {
                errorText = "Couldn't find this Mac in the headphones' paired-device list."
            }
            return
        }

        let audioCount = peers.filter { $0.connected || ($0.isLocal && audioAvailable) }.count
        BarLog.write("connectMacAudio \(local.mac); connected=\(audioCount)/2")
        BarLog.write("asking headphones to connect this Mac (\(local.mac)) for audio")
        let response = try? await exchange(4, 1, op: .start,
                                           payload: [0x00] + ByteCodec.macBytes(local.mac),
                                           timeout: 4.0)
        BarLog.write("CONNECT self response: \(response?.map { $0.op } ?? [])")

        for _ in 0..<12 {
            try? await Task.sleep(nanoseconds: 400_000_000)
            refreshAudioAvailability()
            if audioAvailable { break }
        }

        if !audioAvailable {
            if !automatic {
                errorText = "macOS still isn't offering the headphones as an output. Press the Bluetooth button on the headphones, then try again."
            }
            BarLog.write("CONNECT self did not produce a macOS audio device")
            NSLog("BoseBuddy: CONNECT self did not produce an audio device")
        } else {
            BarLog.write("Mac audio connected")
            NSLog("BoseBuddy: Mac audio connected")
            await readSource()
            await loadPeers()
        }
    }

    func useThisMac() async {
        if let local = peers.first(where: { $0.isLocal }) {
            await connectPeer(local)
        } else {
            await connectMacAudio()
        }
    }

    func disconnectMacAudio() async {
        guard linkState.isConnected else { return }
        busyAction = "audio"
        defer { busyAction = nil }
        errorText = nil
        await loadPeers()
        guard let local = peers.first(where: { $0.isLocal }) else { return }
        actionNote = "Disconnecting this Mac…"
        _ = try? await exchange(4, 2, op: .start,
                                payload: ByteCodec.macBytes(local.mac),
                                expectReply: false, timeout: 2.0)
        for _ in 0..<12 {
            try? await Task.sleep(nanoseconds: 400_000_000)
            refreshAudioAvailability()
            if !audioAvailable { break }
        }
        actionNote = nil
        await loadPeers()
        if audioAvailable {
            errorText = "Couldn't disconnect this Mac's audio. Try the Bluetooth button on the headphones."
        }
    }

    private func readModeIndex() async {
        guard linkState.isConnected else { return }
        if let packet = try? await get(31, 3), let first = packet.payload.first {
            modeIndex = first == 0xFF ? nil : Int(first)
        }
    }

    private func readSource() async {
        guard linkState.isConnected else { return }
        guard let packet = try? await get(5, 1) else { return }
        let bytes = [UInt8](packet.payload)
        guard bytes.count >= 3 else {
            sourceKind = .none
            activePeerMAC = nil
            return
        }
        sourceKind = SourceKind(rawValue: Int(bytes[2])) ?? .none
        if sourceKind == .bluetooth, bytes.count >= 9 {
            activePeerMAC = ByteCodec.formatMAC(Array(bytes[3..<9]))
        } else {
            activePeerMAC = nil
        }
        for index in peers.indices {
            peers[index].active = (peers[index].mac == activePeerMAC)
                && (peers[index].connected || (peers[index].isLocal && audioAvailable))
        }
    }

    func loadPeers() async {
        guard linkState.isConnected else { return }
        do {
            let list = try await get(4, 4)
            let bytes = [UInt8](list.payload)
            var macs: [String] = []
            var index = 1
            while index + 5 < bytes.count {
                let chunk = Array(bytes[index..<(index + 6)])
                if chunk.contains(where: { $0 != 0 }) {
                    macs.append(ByteCodec.formatMAC(chunk))
                }
                index += 6
            }

            var result: [BosePeer] = []
            for mac in macs {
                if let info = try? await readPeerInfo(mac) {
                    result.append(info)
                } else {
                    result.append(BosePeer(mac: mac, name: mac, connected: false,
                                           isLocal: false, isBoseProduct: false))
                }
            }

            result.sort { lhs, rhs in
                if lhs.isLocal != rhs.isLocal { return lhs.isLocal }
                if lhs.connected != rhs.connected { return lhs.connected }
                return lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName) == .orderedAscending
            }
            for idx in result.indices {
                result[idx].active = (result[idx].mac == activePeerMAC)
                    && (result[idx].connected || (result[idx].isLocal && audioAvailable))
            }
            peers = result
            let summary = result.map {
                "\($0.displayName):\($0.connected ? "on" : "off")\($0.isLocal ? "/local" : "")"
            }.joined(separator: ", ")
            BarLog.write("peers: \(summary) source=\(activePeerMAC ?? "-")")
            supportsPeers = true
            peersLoaded = true
        } catch {
            supportsPeers = false
            peersLoaded = true
        }
    }

    private func readPeerInfo(_ mac: String) async throws -> BosePeer {
        let packet = try await get(4, 5, payload: ByteCodec.macBytes(mac))
        let bytes = [UInt8](packet.payload)
        var peer = BosePeer(mac: mac, name: mac, connected: false,
                            isLocal: false, isBoseProduct: false)
        guard bytes.count >= 7 else { return peer }
        peer.connected = (bytes[6] & 0x01) != 0
        peer.isLocal = (bytes[6] & 0x02) != 0
        peer.isBoseProduct = (bytes[6] & 0x04) != 0
        let nameStart = peer.isBoseProduct ? 10 : 9
        if nameStart < bytes.count {
            let name = ByteCodec.string(bytes[nameStart...])
            if !name.isEmpty { peer.name = name }
        }
        return peer
    }

    func loadModes() async {
        guard linkState.isConnected else { return }
        do {
            let packets = try await exchange(31, 1, op: .start, drain: true, timeout: 2.8)
            var found: [Int: BoseMode] = [:]
            for packet in packets where packet.fblock == 31 && packet.function == 6 {
                let bytes = [UInt8](packet.payload)
                guard bytes.count >= 38 else { continue }
                let index = Int(bytes[0])
                guard index != 0xFF else { continue }
                let name = ByteCodec.string(bytes[6..<38])
                if name.isEmpty && index > 1 { continue }
                let cncOffset = bytes.count >= 48 ? 38 : 42
                let rawCNC = bytes.count > cncOffset ? Int(bytes[cncOffset]) : 0
                found[index] = BoseMode(
                    index: index,
                    name: name.isEmpty ? Self.defaultModeName(index) : name,
                    rawCNC: rawCNC
                )
            }
            if found.isEmpty {
                modes = [
                    BoseMode(index: 0, name: "Quiet", rawCNC: 0),
                    BoseMode(index: 1, name: "Aware", rawCNC: 10)
                ]
            } else {
                modes = found.values.sorted { $0.index < $1.index }
            }
            supportsModes = true
        } catch {
            if modes.isEmpty {
                modes = [
                    BoseMode(index: 0, name: "Quiet", rawCNC: 0),
                    BoseMode(index: 1, name: "Aware", rawCNC: 10)
                ]
            }
        }
    }

    func loadEQ() async {
        guard linkState.isConnected else { return }
        do {
            let packet = try await get(1, 7)
            let bytes = [UInt8](packet.payload)
            var bands: [EQBand] = []
            var index = 0
            let names = ["Bass", "Mid", "Treble"]
            while index + 3 < bytes.count {
                let minimum = Int(Int8(bitPattern: bytes[index]))
                let maximum = Int(Int8(bitPattern: bytes[index + 1]))
                let current = Int(Int8(bitPattern: bytes[index + 2]))
                let bandID = Int(bytes[index + 3])
                guard maximum > minimum else {
                    index += 4
                    continue
                }
                bands.append(EQBand(
                    bandID: bandID,
                    name: bandID < names.count ? names[bandID] : "Band \(bandID)",
                    minimum: minimum,
                    maximum: maximum,
                    value: current
                ))
                index += 4
            }
            if !bands.isEmpty {
                eqBands = bands.sorted { $0.bandID < $1.bandID }
                supportsEQ = true
            }
        } catch {
            if eqBands.isEmpty { supportsEQ = false }
        }
    }

    func loadMultipoint() async {
        guard linkState.isConnected else { return }
        do {
            let packet = try await get(1, 10)
            multipoint = ((packet.payload.first ?? 0) & 0x02) != 0
            supportsMultipoint = true
        } catch {
            supportsMultipoint = false
        }
    }

    func loadSidetone() async {
        guard linkState.isConnected else { return }
        do {
            let packet = try await get(1, 11)
            sidetone = Int(packet.payload.count >= 2 ? packet.payload[1] : 0)
            supportsSidetone = true
        } catch {
            supportsSidetone = false
        }
    }

    // MARK: - Actions

    func setMode(_ index: Int) async {
        guard linkState.isConnected else { return }
        busyAction = "mode"
        defer { busyAction = nil }
        let previous = modeIndex
        modeIndex = index
        do {
            let packets = try await exchange(31, 3, op: .start,
                                             payload: [UInt8(index), 0], timeout: 3.0)
            try ensureOK(packets, fblock: 31, function: 3)
            try await Task.sleep(nanoseconds: 250_000_000)
            await readModeIndex()
        } catch {
            modeIndex = previous
            errorText = message(for: error)
        }
    }

    func setMultipoint(_ enabled: Bool) async {
        guard linkState.isConnected else { return }
        busyAction = "multipoint"
        defer { busyAction = nil }
        let previous = multipoint
        multipoint = enabled
        do {
            let packets = try await exchange(1, 10, op: .setGet, payload: [enabled ? 1 : 0])
            try ensureOK(packets, fblock: 1, function: 10)
            try await Task.sleep(nanoseconds: 200_000_000)
            await loadMultipoint()
            await loadPeers()
        } catch {
            multipoint = previous
            errorText = message(for: error)
        }
    }

    func setEQ(band: EQBand, value: Int) async {
        guard linkState.isConnected else { return }
        let clamped = max(band.minimum, min(band.maximum, value))
        busyAction = "eq:\(band.bandID)"
        defer { busyAction = nil }
        if let index = eqBands.firstIndex(where: { $0.bandID == band.bandID }) {
            eqBands[index].value = clamped
        }
        do {
            let packets = try await exchange(1, 7, op: .setGet,
                                             payload: [UInt8(bitPattern: Int8(clamping: clamped)),
                                                       UInt8(band.bandID)])
            try ensureOK(packets, fblock: 1, function: 7)
            await loadEQ()
        } catch {
            errorText = message(for: error)
            await loadEQ()
        }
    }

    func setSidetone(_ level: Int) async {
        guard linkState.isConnected else { return }
        busyAction = "sidetone"
        defer { busyAction = nil }
        sidetone = level
        do {
            let packets = try await exchange(1, 11, op: .setGet, payload: [1, UInt8(level)])
            try ensureOK(packets, fblock: 1, function: 11)
            await loadSidetone()
        } catch {
            errorText = message(for: error)
        }
    }

    func connectPeer(_ peer: BosePeer, force: Bool = false) async {
        guard linkState.isConnected, peerAction == nil, !peer.connected else { return }

        // Count audio devices only. The control link itself does not add a slot.
        let connectedPeers = peers.filter { $0.connected || ($0.isLocal && audioAvailable) }
        let atCapacity = connectedPeers.count >= 2
        let candidates = connectedPeers.filter { $0.mac != peer.mac }

        BarLog.write("connectPeer \(peer.displayName): connected=\(connectedPeers.count)/2 atCapacity=\(atCapacity) candidates=\(candidates.map(\.displayName))")

        if atCapacity {
            if confirmBeforeSwitch, !force {
                pendingSwitch = PeerSwitch(target: peer, candidates: candidates)
                return
            }
            // Prompt disabled: the default is to drop a device that is not this Mac.
            if let victim = candidates.first(where: { !$0.isLocal }) ?? candidates.first {
                peerAction = peer.mac
                actionNote = "Switching from \(victim.displayName)…"
                await performDisconnect(victim)
                await loadPeers()
                peerAction = nil
                actionNote = nil
            }
        }

        await performConnect(peer)
    }

    private func performConnect(_ peer: BosePeer) async {
        guard peerAction == nil else { return }
        peerAction = peer.mac
        defer {
            peerAction = nil
            actionNote = nil
        }
        errorText = nil

        if peer.isLocal {
            await connectMacAudio()
            return
        }

        do {
            actionNote = "Connecting \(peer.displayName)…"
            let packets = try await exchange(4, 1, op: .start,
                                             payload: [0x00] + ByteCodec.macBytes(peer.mac),
                                             timeout: 4.0)
            if let reply = packets.first(where: { $0.fblock == 4 && $0.function == 1 }) ?? packets.first,
               reply.op == .error {
                throw BmapError.device(code: reply.payload.first ?? 0, fblock: 4, function: 1)
            }
            for _ in 0..<14 {
                try await Task.sleep(nanoseconds: 500_000_000)
                if let info = try? await readPeerInfo(peer.mac), info.connected { break }
            }
            await loadPeers()
        } catch {
            errorText = connectMessage(for: error, peer: peer)
        }
    }

    func disconnectPeer(_ peer: BosePeer) async {
        guard linkState.isConnected, peerAction == nil,
              peer.isLocal ? audioAvailable : peer.connected else { return }

        if peer.isLocal {
            await disconnectMacAudio()
            return
        }

        BarLog.write("disconnectPeer \(peer.displayName)")
        peerAction = peer.mac
        defer {
            peerAction = nil
            actionNote = nil
        }
        errorText = nil
        actionNote = "Disconnecting \(peer.displayName)…"
        await performDisconnect(peer)
        await loadPeers()
        if let still = peers.first(where: { $0.mac == peer.mac }), still.connected {
            errorText = "Couldn't disconnect \(peer.displayName). Try again."
        }
    }

    func confirmPendingSwitch(dropping device: BosePeer) {
        guard let pending = pendingSwitch else { return }
        pendingSwitch = nil
        Task {
            peerAction = device.mac
            actionNote = "Disconnecting \(device.displayName)…"
            await performDisconnect(device)
            await loadPeers()
            peerAction = nil
            actionNote = nil
            await connectPeer(pending.target, force: true)
        }
    }

    func cancelPendingSwitch() {
        pendingSwitch = nil
    }

    /// Sends DISCONNECT and waits for the connected flag to clear. Best effort.
    private func performDisconnect(_ peer: BosePeer) async {
        _ = try? await exchange(4, 2, op: .start,
                                payload: ByteCodec.macBytes(peer.mac),
                                expectReply: false, timeout: 2.0)
        for _ in 0..<12 {
            try? await Task.sleep(nanoseconds: 400_000_000)
            if let info = try? await readPeerInfo(peer.mac), !info.connected { break }
        }
    }

    func clearPairingMode() {
        pairingMode = false
    }

    func enterPairingMode() async {
        guard linkState.isConnected else { return }
        busyAction = "pairing"
        defer { busyAction = nil }
        do {
            let packets = try await exchange(4, 8, op: .start, payload: [1], timeout: 3.0)
            try ensureOK(packets, fblock: 4, function: 8)
            pairingMode = true
        } catch {
            errorText = message(for: error)
        }
    }

    func powerOff() async {
        guard linkState.isConnected else { return }
        _ = try? await exchange(7, 4, op: .start, payload: [0], expectReply: false, timeout: 0.5)
        disconnect()
    }

    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
        } catch {
            errorText = "Couldn't update the login item: \(error.localizedDescription)"
        }
        launchAtLogin = SMAppService.mainApp.status == .enabled
    }

    func openBluetoothSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth") else { return }
        NSWorkspace.shared.open(url)
    }

    // MARK: - Packet helpers

    private func exchange(_ fblock: UInt8, _ function: UInt8, op: BmapOperator,
                          payload: [UInt8] = [], port: UInt8 = 0,
                          expectReply: Bool = true, drain: Bool = false,
                          timeout: TimeInterval = 2.8) async throws -> [BmapPacket] {
        let packet = BmapPacket(fblock: fblock, function: function, port: port,
                                op: op, payload: Data(payload))
        guard let link else { throw BmapError.notConnected }
        let data = try await link.send(packet.encoded(), expectReply: expectReply,
                                       drain: drain, timeout: timeout)
        return BmapPacket.parseAll(data)
    }

    private func get(_ fblock: UInt8, _ function: UInt8, payload: [UInt8] = [],
                     timeout: TimeInterval = 2.8) async throws -> BmapPacket {
        let packets = try await exchange(fblock, function, op: .get,
                                         payload: payload, timeout: timeout)
        guard let reply = packets.first(where: { $0.fblock == fblock && $0.function == function })
                ?? packets.first else {
            throw BmapError.noResponse
        }
        if reply.op == .error {
            throw BmapError.device(code: reply.payload.first ?? 0,
                                   fblock: fblock, function: function)
        }
        return reply
    }

    private func ensureOK(_ packets: [BmapPacket], fblock: UInt8, function: UInt8) throws {
        guard let reply = packets.first(where: { $0.fblock == fblock && $0.function == function })
                ?? packets.first else { return }
        if reply.op == .error {
            throw BmapError.device(code: reply.payload.first ?? 0,
                                   fblock: fblock, function: function)
        }
    }

    private func message(for error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }

    private func connectMessage(for error: Error, peer: BosePeer) -> String {
        if case BmapError.device(let code, _, _) = error {
            switch code {
            case 7:
                return "\(peer.displayName) isn't available. Make sure it's nearby and Bluetooth is on."
            case 8:
                return "\(peer.displayName) didn't answer. Make sure it's awake, nearby, and Bluetooth is on."
            case 11:
                return "\(peer.displayName) is no longer paired with the headphones."
            case 12:
                return "The headphones are busy. Try again in a moment."
            default:
                return "Couldn't connect \(peer.displayName) (\(BmapError.name(code)))."
            }
        }
        return message(for: error)
    }

    // MARK: - Utilities

    nonisolated private static func friendlyName(_ raw: String) -> String {
        var name = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if name.hasPrefix("LE-") { name = String(name.dropFirst(3)) }
        return name.isEmpty ? "Bose headphones" : name
    }

    nonisolated private static func looksLikeBose(_ name: String) -> Bool {
        let lower = name.lowercased()
        return lower.contains("bose")
            || lower.contains("quietcomfort")
            || lower.contains("qc35")
            || lower.contains("qc45")
            || lower.contains("qc ultra")
    }

    private static func channelCandidates(for name: String) -> [UInt8] {
        let lower = name.lowercased()
        if lower.contains("ultra") || lower.contains("700") {
            return [2, 8, 9]
        }
        return [8, 2, 9]
    }

    private static func defaultModeName(_ index: Int) -> String {
        switch index {
        case 0: return "Quiet"
        case 1: return "Aware"
        case 2: return "Immersion"
        case 3: return "Cinema"
        default: return "Mode \(index + 1)"
        }
    }

    /// Debug helper: clears the non-local multipoint slots, then connects a target.
    func runSwitchTest(to fragment: String) async {
        guard linkState.isConnected else {
            print("switch test: not connected")
            return
        }
        await loadPeers()
        print("before:\n\(debugSummary)")
        guard let target = peers.first(where: {
            $0.displayName.lowercased().contains(fragment.lowercased())
        }) else {
            print("switch test: no peer matching '\(fragment)'")
            return
        }
        print("connecting \(target.displayName)… (confirm=\(confirmBeforeSwitch))")
        await connectPeer(target)
        if let pending = pendingSwitch {
            print("prompt candidates: \(pending.candidates.map(\.displayName))")
            if let victim = pending.candidates.first {
                confirmPendingSwitch(dropping: victim)
                try? await Task.sleep(nanoseconds: 6_000_000_000)
            }
        }
        print("error: \(errorText ?? "none")")
        await loadPeers()
        print("after:\n\(debugSummary)")
    }

    /// Exercises the connection-management path. Used by the `--selftest --actions`
    /// command line mode and safe to ignore in normal use.
    func runActionTest() async {
        guard linkState.isConnected else {
            print("action test: not connected")
            return
        }
        print("--- action test ---")
        if let target = peers.first(where: { peer in
            !peer.isLocal && !peer.connected &&
            (peer.name.lowercased().contains("ipad") || peer.name.lowercased().contains("pad"))
        }) ?? peers.first(where: { !$0.isLocal && !$0.connected }) {
            print("connecting \(target.displayName) (\(target.mac))…")
            await connectPeer(target)
            if let error = errorText {
                print("connect error: \(error)")
                errorText = nil
            }
            print("connected state: \(peers.first(where: { $0.mac == target.mac })?.connected ?? false)")
            if let updated = peers.first(where: { $0.mac == target.mac }), updated.connected {
                print("disconnecting \(updated.displayName)…")
                await disconnectPeer(updated)
                print("connected state: \(peers.first(where: { $0.mac == target.mac })?.connected ?? false)")
            }
        } else {
            print("no disconnected peer to test")
        }

        print("mode switch → 1 (Aware)")
        await setMode(1)
        try? await Task.sleep(nanoseconds: 400_000_000)
        print("mode now: \(modeIndex.map(String.init) ?? "-")")
        print("mode switch → 0 (Quiet)")
        await setMode(0)
        try? await Task.sleep(nanoseconds: 400_000_000)
        print("mode now: \(modeIndex.map(String.init) ?? "-")")
        print("--- end action test ---")
    }

    /// Populates representative state for UI snapshots without hardware.
    func loadDemoState() {
        deviceName = "Bose QC Headphones"
        productName = "Bose QC Headphones"
        firmware = "1.0.6-80+f5f219b"
        linkState = .connected
        battery = 30
        modes = [
            BoseMode(index: 0, name: "Quiet", rawCNC: 0),
            BoseMode(index: 1, name: "Aware", rawCNC: 10)
        ]
        modeIndex = 0
        multipoint = true
        eqBands = [
            EQBand(bandID: 0, name: "Bass", minimum: -10, maximum: 10, value: 0),
            EQBand(bandID: 1, name: "Mid", minimum: -10, maximum: 10, value: 0),
            EQBand(bandID: 2, name: "Treble", minimum: -10, maximum: 10, value: 0)
        ]
        peers = [
            BosePeer(mac: "50:ED:3C:07:78:DE", name: "MacBook Air", connected: false,
                     isLocal: true, isBoseProduct: false),
            BosePeer(mac: "04:2E:C1:4E:77:3D", name: "Anmol iPhone", connected: true,
                     isLocal: false, isBoseProduct: false, active: true),
            BosePeer(mac: "00:16:A4:D0:41:BE", name: "00:16:A4:D0:41:BE", connected: false,
                     isLocal: false, isBoseProduct: false),
            BosePeer(mac: "60:3E:5F:5F:2B:73", name: "603e5f68b6b8", connected: false,
                     isLocal: false, isBoseProduct: false),
            BosePeer(mac: "60:F5:49:25:AC:E1", name: "aPad", connected: false,
                     isLocal: false, isBoseProduct: false)
        ]
        activePeerMAC = "04:2E:C1:4E:77:3D"
        audioAvailable = true
        for index in peers.indices where peers[index].isLocal {
            peers[index].connected = true
        }
        supportsModes = true
        supportsEQ = true
        supportsMultipoint = true
        supportsPeers = true
        peersLoaded = true
    }

    func loadDemoSwitchState() {
        loadDemoState()
        audioAvailable = true
        for index in peers.indices where peers[index].isLocal {
            peers[index].connected = true
        }
        guard let target = peers.first(where: { $0.displayName == "aPad" }) else { return }
        let candidates = peers.filter { $0.connected }
        pendingSwitch = PeerSwitch(target: target, candidates: candidates)
    }

    var debugSummary: String {
        var lines: [String] = []
        lines.append("connection: \(linkState) channel=\(channelID)")
        lines.append("name: \(productName.isEmpty ? deviceName : productName)")
        lines.append("firmware: \(firmware)")
        lines.append("battery: \(battery.map { "\($0)%" } ?? "?")\(charging ? " (charging)" : "")")
        lines.append("mode: \(modeIndex.map(String.init) ?? "-") of \(modes.map { "\($0.index)=\($0.name)" }.joined(separator: ", "))")
        lines.append("multipoint: \(multipoint) (supported: \(supportsMultipoint))")
        lines.append("eq: \(eqBands.map { "\($0.name)=\($0.value)" }.joined(separator: ", "))")
        lines.append("source: \(sourceKind) \(activePeerMAC ?? "")")
        lines.append("audio: \(audioAvailable)")
        lines.append("peers:")
        for peer in peers {
            lines.append("  \(peer.mac)  \(peer.displayName)  connected=\(peer.connected) local=\(peer.isLocal) active=\(peer.active)")
        }
        return lines.joined(separator: "\n")
    }
}
