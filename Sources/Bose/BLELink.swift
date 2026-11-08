import CoreBluetooth
import Foundation

enum BLELinkError: LocalizedError {
    case bluetoothOff
    case unauthorized
    case notFound
    case connectFailed(String)
    case noBMAPService
    case needsBonding
    case writeFailed(String)
    case timeout
    case notConnected
    case busy

    var errorDescription: String? {
        switch self {
        case .bluetoothOff:
            return "Bluetooth is off."
        case .unauthorized:
            return "Bluetooth permission is required. Enable BoseBuddy in System Settings → Privacy & Security → Bluetooth."
        case .notFound:
            return "Couldn’t find the headphones over Bluetooth LE. Make sure they’re on and close the Bose app on your phone."
        case .connectFailed(let message):
            return "Couldn’t connect over Bluetooth LE: \(message)"
        case .noBMAPService:
            return "The headphones don’t expose the Bose BLE control service."
        case .needsBonding:
            return "Bluetooth LE pairing is required. Put the headphones in pairing mode, then retry."
        case .writeFailed(let message):
            return "Bluetooth LE write failed: \(message)"
        case .timeout:
            return "The headphones didn’t answer over Bluetooth LE."
        case .notConnected:
            return "Bluetooth LE control is not connected."
        case .busy:
            return "Another Bluetooth LE request is still running."
        }
    }
}

/// BMAP over the Bose BLE service. This is the primary control path: it does
/// not consume one of the headphones' two classic/multipoint audio slots.
final class BLELink: NSObject, CBCentralManagerDelegate, CBPeripheralDelegate {
    static let serviceUUID = CBUUID(string: "0000FEBE-0000-1000-8000-00805F9B34FB")
    static let secureUUID = CBUUID(string: "C65B8F2F-AEE2-4C89-B758-BC4892D6F2D8")
    static let unsecureUUID = CBUUID(string: "D417C028-9818-4354-99D1-2AC09D074591")

    private var central: CBCentralManager?
    private var peripheral: CBPeripheral?
    private var characteristic: CBCharacteristic?
    private var writeType: CBCharacteristicWriteType = .withResponse
    private var assembler = BLESegmentAssembler()

    private var openContinuation: CheckedContinuation<Void, Error>?
    private var responseContinuation: CheckedContinuation<Data, Error>?
    private var responseBuffer = Data()
    private var expected: (UInt8, UInt8)?
    private var drain = false
    private var drainWork: DispatchWorkItem?
    private var timeoutWork: DispatchWorkItem?
    private var openTimeoutWork: DispatchWorkItem?
    private var verifying = false
    private var opening = false
    private var closing = false

    var onClosed: (() -> Void)?

    var isOpen: Bool {
        peripheral?.state == .connected && characteristic != nil
    }

    // MARK: - Open

    func open() async throws {
        guard !isOpen else { return }
        closing = false
        try await withCheckedThrowingContinuation { continuation in
            self.openContinuation = continuation
            if let central {
                self.central = central
                if central.state == .poweredOn {
                    self.startScan()
                } else if central.state == .unauthorized {
                    self.failOpen(.unauthorized)
                } else if central.state == .poweredOff {
                    self.failOpen(.bluetoothOff)
                }
            } else {
                self.central = CBCentralManager(delegate: self, queue: nil)
            }
            self.armOpenTimeout()
        }
    }

    private func startScan() {
        guard !opening, let central else { return }
        opening = true
        BarLog.write("BLE: scanning for headphones")
        central.scanForPeripherals(withServices: nil,
                                   options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
    }

    private func armOpenTimeout() {
        openTimeoutWork?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.failOpen(.notFound)
        }
        openTimeoutWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 15, execute: work)
    }

    private func finishOpen() {
        openTimeoutWork?.cancel()
        openTimeoutWork = nil
        opening = false
        guard let continuation = openContinuation else { return }
        openContinuation = nil
        continuation.resume()
    }

    private func failOpen(_ error: BLELinkError) {
        openTimeoutWork?.cancel()
        openTimeoutWork = nil
        opening = false
        if let central {
            central.stopScan()
        }
        if let peripheral {
            central?.cancelPeripheralConnection(peripheral)
        }
        peripheral = nil
        characteristic = nil
        guard let continuation = openContinuation else { return }
        openContinuation = nil
        continuation.resume(throwing: error)
    }

    // MARK: - Send

    func send(_ packet: Data, expectReply: Bool = true, drain: Bool = false,
              timeout: TimeInterval = 2.5) async throws -> Data {
        guard let peripheral, let characteristic, peripheral.state == .connected else {
            throw BLELinkError.notConnected
        }
        guard responseContinuation == nil else { throw BLELinkError.busy }

        let bytes = [UInt8](packet)
        expected = bytes.count >= 2 ? (bytes[0], bytes[1]) : nil
        self.drain = drain
        responseBuffer.removeAll()

        let frames = Self.frames(for: bytes)

        guard expectReply else {
            for frame in frames {
                peripheral.writeValue(frame, for: characteristic, type: writeType)
            }
            try? await Task.sleep(nanoseconds: 120_000_000)
            return Data()
        }

        // Install the continuation before writing: a fast BLE notification must
        // not arrive before we are listening for it.
        return try await withCheckedThrowingContinuation { continuation in
            self.responseContinuation = continuation
            let work = DispatchWorkItem { [weak self] in
                self?.finishResponse()
            }
            self.timeoutWork = work
            DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: work)
            for frame in frames {
                peripheral.writeValue(frame, for: characteristic, type: writeType)
            }
        }
    }

    private func handleNotification(_ data: Data) {
        guard let assembled = assembler.feed(data) else { return }
        BarLog.write("BLE: rx \(data.count) bytes → BMAP \(assembled.count)")

        if verifying {
            verifying = false
            finishOpen()
            return
        }

        guard responseContinuation != nil else { return }

        if drain {
            responseBuffer.append(assembled)
            drainWork?.cancel()
            let work = DispatchWorkItem { [weak self] in self?.finishResponse() }
            drainWork = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: work)
            return
        }

        let packets = BmapPacket.parseAll(assembled)
        if let expected,
           !packets.contains(where: { $0.fblock == expected.0 && $0.function == expected.1 }) {
            return
        }
        responseBuffer = assembled
        finishResponse()
    }

    private func finishResponse() {
        timeoutWork?.cancel()
        timeoutWork = nil
        drainWork?.cancel()
        drainWork = nil
        expected = nil
        guard let continuation = responseContinuation else { return }
        responseContinuation = nil
        if responseBuffer.isEmpty {
            continuation.resume(throwing: BLELinkError.timeout)
        } else {
            let data = responseBuffer
            responseBuffer = Data()
            continuation.resume(returning: data)
        }
    }

    private func failPending(_ error: Error) {
        timeoutWork?.cancel()
        timeoutWork = nil
        drainWork?.cancel()
        drainWork = nil
        expected = nil
        responseBuffer.removeAll()
        guard let continuation = responseContinuation else { return }
        responseContinuation = nil
        continuation.resume(throwing: error)
    }

    func close() {
        closing = true
        openTimeoutWork?.cancel()
        if let central {
            central.stopScan()
        }
        if let peripheral, let central {
            central.cancelPeripheralConnection(peripheral)
        }
        peripheral = nil
        characteristic = nil
        assembler = BLESegmentAssembler()
        if openContinuation != nil {
            failOpen(BLELinkError.notConnected)
        }
        failPending(BLELinkError.notConnected)
    }

    // MARK: - Central delegate

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        BarLog.write("BLE: central state=\(central.state.rawValue) auth=\(CBCentralManager.authorization.rawValue)")
        switch central.state {
        case .poweredOn:
            if openContinuation != nil, !opening {
                startScan()
            }
        case .unauthorized:
            failOpen(.unauthorized)
        case .poweredOff:
            failOpen(.bluetoothOff)
        default:
            break
        }
    }

    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
                        advertisementData: [String: Any], rssi RSSI: NSNumber) {
        let name = (advertisementData[CBAdvertisementDataLocalNameKey] as? String)
            ?? peripheral.name ?? ""
        let services = (advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID]) ?? []
        let lower = name.lowercased()
        let looksBose = lower.contains("bose") || lower.contains("adjuster")
        let hasService = services.contains(Self.serviceUUID)
        // Only the named "LE-Bose …" identity carries the usable bond. An
        // unnamed FEBE advertiser is a stale/duplicate identity that rejects
        // control with InsecureTransport.
        guard looksBose else { return }

        BarLog.write("BLE: found \"\(name)\" rssi=\(RSSI) service=\(hasService)")
        central.stopScan()
        self.peripheral = peripheral
        peripheral.delegate = self
        central.connect(peripheral, options: nil)
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        BarLog.write("BLE: connected, discovering services")
        peripheral.discoverServices(nil)
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        failOpen(.connectFailed(error?.localizedDescription ?? "unknown error"))
    }

    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        let wasClosing = closing
        closing = false
        characteristic = nil
        self.peripheral = nil
        failPending(BLELinkError.notConnected)
        if openContinuation != nil {
            failOpen(.connectFailed(error?.localizedDescription ?? "disconnected"))
        }
        if !wasClosing {
            BarLog.write("BLE: disconnected unexpectedly")
            onClosed?()
        }
    }

    // MARK: - Peripheral delegate

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        if let error {
            failOpen(.connectFailed(error.localizedDescription))
            return
        }
        let services = peripheral.services ?? []
        BarLog.write("BLE: services \(services.map(\.uuid.uuidString))")
        guard let bmap = services.first(where: { $0.uuid == Self.serviceUUID }) else {
            failOpen(.noBMAPService)
            return
        }
        peripheral.discoverCharacteristics(nil, for: bmap)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        if let error {
            failOpen(.connectFailed(error.localizedDescription))
            return
        }
        let chars = service.characteristics ?? []
        for characteristic in chars {
            BarLog.write("BLE: char \(characteristic.uuid.uuidString) props=\(characteristic.properties.rawValue)")
        }
        guard let characteristic = chars.first(where: { $0.uuid == Self.secureUUID })
                ?? chars.first(where: { $0.uuid == Self.unsecureUUID }) else {
            failOpen(.noBMAPService)
            return
        }
        self.characteristic = characteristic
        writeType = characteristic.properties.contains(.write) ? .withResponse : .withoutResponse
        BarLog.write("BLE: using \(characteristic.uuid.uuidString), writeType=\(writeType == .withResponse ? "response" : "no response")")
        peripheral.setNotifyValue(true, for: characteristic)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic,
                    error: Error?) {
        if let error {
            failOpen(.connectFailed(error.localizedDescription))
            return
        }
        // Prove the link is actually usable (and bonded) before declaring success.
        verifying = true
        let probe: [UInt8] = [0x02, 0x02, 0x01, 0x00]
        for frame in Self.frames(for: probe) {
            peripheral.writeValue(frame, for: characteristic, type: writeType)
        }
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.verifying else { return }
            self.verifying = false
            self.failOpen(.timeout)
        }
        openTimeoutWork?.cancel()
        openTimeoutWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 9, execute: work)
    }

    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        guard let error else { return }
        let message = error.localizedDescription
        BarLog.write("BLE: write error: \(message)")
        let mapped: BLELinkError = message.lowercased().contains("encrypt") || message.lowercased().contains("authentic")
            ? .needsBonding
            : .writeFailed(message)
        if verifying {
            verifying = false
            failOpen(mapped)
        } else {
            failPending(mapped)
        }
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        if let error {
            BarLog.write("BLE: notify error: \(error.localizedDescription)")
            return
        }
        guard let data = characteristic.value, !data.isEmpty else { return }
        handleNotification(data)
    }

    // MARK: - Framing

    private static func frames(for packet: [UInt8]) -> [Data] {
        let chunkSize = 19
        if packet.count <= chunkSize {
            return [Data([0x00] + packet)]
        }
        let total = (packet.count + chunkSize - 1) / chunkSize
        let maxIndex = UInt8(total - 1)
        var result: [Data] = []
        for index in 0..<total {
            let start = index * chunkSize
            let end = min(start + chunkSize, packet.count)
            var frame = Data([(maxIndex << 4) | UInt8(index)])
            frame.append(contentsOf: packet[start..<end])
            result.append(frame)
        }
        return result
    }
}

private final class BLESegmentAssembler {
    private var segments: [Data?] = []
    private var expected = 0

    func feed(_ segment: Data) -> Data? {
        guard let header = segment.first else { return nil }
        if header == 0x00 {
            return segment.dropFirst()
        }
        let maxIndex = Int((header >> 4) & 0x0F)
        let index = Int(header & 0x0F)
        if expected == 0 {
            expected = maxIndex + 1
            segments = Array(repeating: nil, count: expected)
        }
        guard index < segments.count else {
            reset()
            return nil
        }
        segments[index] = segment.dropFirst()
        guard segments.allSatisfy({ $0 != nil }) else { return nil }
        var result = Data()
        for part in segments {
            if let part { result.append(part) }
        }
        reset()
        return result
    }

    private func reset() {
        segments = []
        expected = 0
    }
}
