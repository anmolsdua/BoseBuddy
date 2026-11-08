import AppKit
import CoreBluetooth
import Foundation

/// Proof-of-concept BLE BMAP probe. Scans for the Bose FEBE service, connects,
/// enables notifications and exercises a few control/device-management reads.
/// Results go to /tmp/bosebuddy-ble.log and ~/Library/Logs/BoseBuddy.log.
final class BLEProbe: NSObject, CBCentralManagerDelegate, CBPeripheralDelegate {
    private static let bmapService = CBUUID(string: "0000FEBE-0000-1000-8000-00805F9B34FB")
    private static let secureChar = CBUUID(string: "C65B8F2F-AEE2-4C89-B758-BC4892D6F2D8")
    private static let unsecureChar = CBUUID(string: "D417C028-9818-4354-99D1-2AC09D074591")

    private var central: CBCentralManager!
    private var target: CBPeripheral?
    private var bmapCharacteristic: CBCharacteristic?
    private var writeType: CBCharacteristicWriteType = .withoutResponse
    private var assembler = BLEAssembler()
    private var seen = Set<UUID>()
    private var finished = false
    private var requestsSent = 0
    private var didRunConnectTest = false
    private var didRunMacConnectTest = false

    func start() {
        try? "".write(to: URL(fileURLWithPath: "/tmp/bosebuddy-ble.log"), atomically: true, encoding: .utf8)
        log("BLE probe starting (45s budget)")
        central = CBCentralManager(delegate: self, queue: nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 45) { [weak self] in
            self?.finish("timeout")
        }
    }

    // MARK: - Central

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        log("central state=\(central.state.rawValue) auth=\(CBCentralManager.authorization.rawValue)")
        switch central.state {
        case .poweredOn:
            central.scanForPeripherals(withServices: nil,
                                       options: [CBCentralManagerScanOptionAllowDuplicatesKey: true])
            log("scanning for all peripherals")
        case .unauthorized:
            finish("Bluetooth unauthorized — enable BoseBuddy in System Settings → Privacy → Bluetooth")
        case .poweredOff:
            finish("Bluetooth is off")
        default:
            break
        }
    }

    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
                        advertisementData: [String: Any], rssi RSSI: NSNumber) {
        let name = (advertisementData[CBAdvertisementDataLocalNameKey] as? String)
            ?? peripheral.name ?? ""
        let services = (advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID]) ?? []
        let serviceNames = services.map(\.uuidString)

        if !seen.contains(peripheral.identifier) {
            seen.insert(peripheral.identifier)
            log("seen \"\(name)\" id=\(peripheral.identifier.uuidString) rssi=\(RSSI) services=\(serviceNames)")
        }

        let lower = name.lowercased()
        let looksBose = lower.contains("bose") || lower.contains("adjuster") || lower.contains("qc")
        let hasService = services.contains(Self.bmapService)
        guard target == nil, looksBose || hasService else { return }

        target = peripheral
        central.stopScan()
        log("candidate: \"\(name)\" byService=\(hasService) byName=\(looksBose); connecting")
        peripheral.delegate = self
        central.connect(peripheral, options: nil)
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        log("connected to \(peripheral.name ?? "?")")
        peripheral.discoverServices(nil)
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        log("connect failed: \(error?.localizedDescription ?? "unknown")")
        finish("connect failed")
    }

    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        log("disconnected: \(error?.localizedDescription ?? "no error")")
    }

    // MARK: - Peripheral

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        if let error { log("discoverServices error: \(error.localizedDescription)") }
        let services = peripheral.services ?? []
        log("services: \(services.map(\.uuid.uuidString))")
        for service in services {
            peripheral.discoverCharacteristics(nil, for: service)
        }
        if services.isEmpty {
            finish("no services")
        }
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        if let error { log("discoverCharacteristics error: \(error.localizedDescription)") }
        let characteristics = service.characteristics ?? []
        for characteristic in characteristics {
            log("char \(characteristic.uuid.uuidString) props=\(characteristic.properties.rawValue) service=\(service.uuid.uuidString)")
        }

        guard bmapCharacteristic == nil else { return }
        let chars = characteristics
        // The unsecure characteristic answers InsecureTransport for BMAP
        // control, so use the encrypted characteristic first.
        if let characteristic = chars.first(where: { $0.uuid == Self.secureChar })
            ?? chars.first(where: { $0.uuid == Self.unsecureChar }) {
            bmapCharacteristic = characteristic
            writeType = characteristic.properties.contains(.write) ? .withResponse : .withoutResponse
            log("using BMAP char \(characteristic.uuid.uuidString), writeType=\(writeType == .withoutResponse ? "withoutResponse" : "withResponse")")
            peripheral.setNotifyValue(true, for: characteristic)
        }
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic,
                    error: Error?) {
        if let error {
            log("notify error: \(error.localizedDescription)")
            finish("could not subscribe")
            return
        }
        log("notifications enabled on \(characteristic.uuid.uuidString)")
        scheduleRequests()
    }

    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?) {
        if let error {
            log("write error: \(error.localizedDescription)")
        } else {
            log("write acknowledged")
        }
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard let data = characteristic.value, !data.isEmpty else { return }
        log("notify \(hex(data))")
        if let packet = assembler.feed(data) {
            log("BMAP \(hex(packet))")
            for parsed in BmapPacket.parseAll(packet) {
                handle(parsed)
            }
        }
    }

    // MARK: - Requests

    private func scheduleRequests() {
        let requests: [(String, [UInt8], TimeInterval)] = [
            ("battery [2.2]", [0x02, 0x02, 0x01, 0x00], 0.4),
            ("multipoint [1.10]", [0x01, 0x0A, 0x01, 0x00], 2.2),
            ("paired devices [4.4]", [0x04, 0x04, 0x01, 0x00], 4.2),
            ("source [5.1]", [0x05, 0x01, 0x01, 0x00], 7.5)
        ]
        for (label, packet, delay) in requests {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                self?.send(packet, label: label)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 16) { [weak self] in
            self?.finish("probe complete")
        }
    }

    private func send(_ packet: [UInt8], label: String) {
        guard let peripheral = target, let characteristic = bmapCharacteristic else { return }
        requestsSent += 1
        let frames = Self.frames(for: packet)
        log("send \(label): \(hex(Data(packet)))")
        for frame in frames {
            peripheral.writeValue(frame, for: characteristic, type: writeType)
        }
    }

    private func handle(_ packet: BmapPacket) {
        log("packet [\(packet.fblock).\(packet.function)] op=\(packet.op.rawValue) payload=\(hex(packet.payload))")
        if packet.op == .error {
            log("  device error: \(BmapError.name(packet.payload.first ?? 0))")
            return
        }

        if packet.fblock == 0x04 && packet.function == 0x04 {
            let bytes = [UInt8](packet.payload)
            var macs: [String] = []
            var index = 1
            while index + 5 < bytes.count {
                let mac = Array(bytes[index..<(index + 6)])
                if mac.contains(where: { $0 != 0 }) {
                    macs.append(ByteCodec.formatMAC(mac))
                }
                index += 6
            }
            log("paired MACs: \(macs)")
            for (offset, mac) in macs.enumerated() {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4 * Double(offset + 1)) { [weak self] in
                    self?.send([0x04, 0x05, 0x01, 0x06] + ByteCodec.macBytes(mac),
                               label: "device info \(mac)")
                }
            }
        }

        if packet.fblock == 0x04 && packet.function == 0x05 {
            let bytes = [UInt8](packet.payload)
            if bytes.count >= 9 {
                let isBose = (bytes[6] & 0x04) != 0
                let start = isBose ? 10 : 9
                let name = start < bytes.count ? ByteCodec.string(bytes[start...]) : ""
                let mac = Array(bytes[0..<6])
                log("device info: mac=\(ByteCodec.formatMAC(mac)) flags=\(bytes[6]) name=\"\(name)\"")

                if !didRunMacConnectTest, (bytes[6] & 0x02) != 0, (bytes[6] & 0x01) == 0 {
                    didRunMacConnectTest = true
                    log("MGMT TEST: local Mac is not connected; CONNECT over BLE")
                    send([0x04, 0x01, 0x05, 0x07, 0x00] + mac, label: "CONNECT local Mac")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac, label: "info after Mac connect")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 4.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac, label: "info after Mac connect 2")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 7.0) { [weak self] in
                        self?.send([0x04, 0x02, 0x05, 0x06] + mac, label: "DISCONNECT local Mac")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 9.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac, label: "info after Mac disconnect")
                    }
                }

                if !didRunConnectTest, name.lowercased().contains("iphone"),
                   (bytes[6] & 0x01) == 1 {
                    didRunConnectTest = true
                    log("MGMT TEST: \(name) is connected; DISCONNECT over BLE")
                    send([0x04, 0x02, 0x05, 0x06] + mac,
                         label: "DISCONNECT \(name)")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac,
                                   label: "info after disconnect \(name)")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac,
                                   label: "info after disconnect 2 \(name)")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 5.0) { [weak self] in
                        self?.log("MGMT TEST: CONNECT \(name) over BLE")
                        self?.send([0x04, 0x01, 0x05, 0x07, 0x00] + mac,
                                   label: "CONNECT \(name)")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 7.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac,
                                   label: "info after connect \(name)")
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 9.0) { [weak self] in
                        self?.send([0x04, 0x05, 0x01, 0x06] + mac,
                                   label: "info after connect 2 \(name)")
                    }
                }
            }
        }

        if packet.fblock == 0x05 && packet.function == 0x01 {
            let bytes = [UInt8](packet.payload)
            if bytes.count >= 9, bytes[2] == 1 {
                log("active source: bluetooth \(ByteCodec.formatMAC(Array(bytes[3..<9])))")
            } else {
                log("active source: type=\(bytes.count > 2 ? bytes[2] : 255)")
            }
        }

        if packet.fblock == 0x01 && packet.function == 0x0A {
            let enabled = ((packet.payload.first ?? 0) & 0x02) != 0
            log("multipoint: \(enabled)")
        }

        if packet.fblock == 0x02 && packet.function == 0x02 {
            log("battery: \(packet.payload.first ?? 0)%")
        }
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

    // MARK: - Finishing

    private func finish(_ reason: String) {
        guard !finished else { return }
        finished = true
        central?.stopScan()
        log("FINISHED: \(reason) (seen \(seen.count) peripherals, sent \(requestsSent) requests)")
        if let target {
            central?.cancelPeripheralConnection(target)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
            NSApp.terminate(nil)
        }
    }

    private func log(_ message: String) {
        let line = "\(ISO8601DateFormatter().string(from: Date())) \(message)\n"
        let url = URL(fileURLWithPath: "/tmp/bosebuddy-ble.log")
        if let data = line.data(using: .utf8) {
            if let handle = try? FileHandle(forWritingTo: url) {
                handle.seekToEndOfFile()
                handle.write(data)
                try? handle.close()
            } else {
                try? data.write(to: url)
            }
        }
        BarLog.write("BLE: \(message)")
    }

    private func hex(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined(separator: " ")
    }
}

/// Reassembles Bose's 1-byte-header BLE segmentation.
private final class BLEAssembler {
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
