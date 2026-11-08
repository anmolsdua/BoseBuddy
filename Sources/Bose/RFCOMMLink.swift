import Foundation
import IOBluetooth

/// A dedicated thread that owns the IOBluetooth run loop.
/// All RFCOMM work happens here so the UI never blocks.
private final class RunLoopBox {
    var runLoop: CFRunLoop?
    let ready = DispatchSemaphore(value: 0)
}

private final class IOThread {
    private let box: RunLoopBox
    private let thread: Thread

    init(name: String) {
        let box = RunLoopBox()
        self.box = box
        let thread = Thread {
            box.runLoop = CFRunLoopGetCurrent()
            // Keep the run loop alive even when no sources are attached.
            let keepAlive = Timer(timeInterval: 3600, repeats: true) { _ in }
            RunLoop.current.add(keepAlive, forMode: .default)
            box.ready.signal()
            CFRunLoopRun()
        }
        self.thread = thread
        thread.name = name
        thread.stackSize = 512 * 1024
        thread.start()
        box.ready.wait()
    }

    func perform(_ block: @escaping () -> Void) {
        guard let runLoop = box.runLoop else { return }
        CFRunLoopPerformBlock(runLoop, CFRunLoopMode.defaultMode.rawValue, block)
        CFRunLoopWakeUp(runLoop)
    }
}

/// Raw BMAP transport over Bluetooth RFCOMM (the classic SPP channel).
/// Commands are serialized on a private thread; receive callbacks arrive on
/// that thread's run loop, which is pumped while waiting for a reply.
final class RFCOMMLink: NSObject, IOBluetoothRFCOMMChannelDelegate, @unchecked Sendable {
    private let io = IOThread(name: "com.bosebuddy.rfcomm")
    private var device: IOBluetoothDevice?
    private var channel: IOBluetoothRFCOMMChannel?
    private var buffer = Data()
    private var basebandConnected = false
    private var channelOpened = false
    private var closed = false
    private var openedBaseband = false
    private(set) var channelID: UInt8 = 0
    /// Called on the IO thread when the peer closes the RFCOMM channel.
    var onClosed: (() -> Void)?

    // MARK: - Async API

    func open(address: String, channels: [UInt8], timeout: TimeInterval = 8) async throws -> UInt8 {
        try await withCheckedThrowingContinuation { continuation in
            io.perform {
                do {
                    let opened = try self.openSync(address: address, channels: channels, timeout: timeout)
                    continuation.resume(returning: opened)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func send(_ packet: Data, expectReply: Bool = true, drain: Bool = false,
              timeout: TimeInterval = 1.5) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            io.perform {
                do {
                    let data = try self.sendSync(packet, expectReply: expectReply,
                                                drain: drain, timeout: timeout)
                    continuation.resume(returning: data)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func close() {
        io.perform { self.closeSync() }
    }

    // MARK: - IO thread

    private func openSync(address: String, channels: [UInt8], timeout: TimeInterval) throws -> UInt8 {
        closeSync()
        closed = false
        let normalized = address.replacingOccurrences(of: ":", with: "-").lowercased()
        guard let device = IOBluetoothDevice(addressString: normalized) else {
            throw BmapError.transport("Headphones not found. Pair them in Bluetooth settings first.")
        }
        self.device = device

        guard device.isConnected() else {
            throw BmapError.transport("Connect the headphones to this Mac in System Settings → Bluetooth first.")
        }

        for candidate in channels {
            channelOpened = false
            var rfcomm: IOBluetoothRFCOMMChannel?
            let result = device.openRFCOMMChannelSync(&rfcomm, withChannelID: candidate,
                                                      delegate: self)
            guard result == kIOReturnSuccess else { continue }
            _ = pump(until: { self.channelOpened }, timeout: 2.5)
            if channelOpened, let rfcomm {
                channel = rfcomm
                channelID = candidate
                pump(0.15)
                buffer.removeAll()
                return candidate
            }
            if let rfcomm { _ = rfcomm.close() }
        }

        cleanupAfterFailedOpen()
        throw BmapError.transport("Connected to the headphones but couldn't open the Bluetooth channel.")
    }

    private func sendSync(_ packet: Data, expectReply: Bool, drain: Bool,
                          timeout: TimeInterval) throws -> Data {
        guard let channel else { throw BmapError.notConnected }
        buffer.removeAll()

        var bytes = [UInt8](packet)
        let status = channel.writeSync(&bytes, length: UInt16(bytes.count))
        guard status == kIOReturnSuccess else {
            throw BmapError.transport("Bluetooth write failed (status \(status)).")
        }
        guard expectReply else {
            pump(0.12)
            return Data()
        }

        let deadline = Date().addingTimeInterval(timeout)
        var sawData = false
        var lastData = Date()

        while Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.008))
            if !buffer.isEmpty {
                if !sawData {
                    sawData = true
                }
                lastData = Date()
                if !drain, isComplete(buffer) { break }
            }
            if drain, sawData, Date().timeIntervalSince(lastData) > 0.3 { break }
            if closed { break }
        }

        guard !buffer.isEmpty else {
            if closed { throw BmapError.notConnected }
            throw BmapError.timeout
        }
        let data = buffer
        buffer = Data()
        return data
    }

    private func closeSync() {
        if let channel {
            _ = channel.close()
        }
        channel = nil
        if openedBaseband, let device {
            _ = device.closeConnection()
        }
        device = nil
        openedBaseband = false
        buffer.removeAll()
        closed = true
    }

    private func cleanupAfterFailedOpen() {
        if let channel {
            _ = channel.close()
        }
        channel = nil
        if openedBaseband, let device {
            _ = device.closeConnection()
        }
        device = nil
        openedBaseband = false
        buffer.removeAll()
    }

    // MARK: - Run loop helpers

    @discardableResult
    private func pump(until condition: () -> Bool, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.008))
        }
        return condition()
    }

    private func pump(_ seconds: TimeInterval) {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.008))
        }
    }

    private func isComplete(_ data: Data) -> Bool {
        let bytes = [UInt8](data)
        var index = 0
        var count = 0
        while index + 4 <= bytes.count {
            let length = Int(bytes[index + 3])
            let end = index + 4 + length
            guard end <= bytes.count else { return false }
            index = end
            count += 1
        }
        return count > 0 && index == bytes.count
    }

    // MARK: - IOBluetooth delegates

    @objc func connectionComplete(_ device: IOBluetoothDevice, status: IOReturn) {
        basebandConnected = (status == kIOReturnSuccess)
    }

    func rfcommChannelOpenComplete(_ rfcommChannel: IOBluetoothRFCOMMChannel!, status: IOReturn) {
        channelOpened = (status == kIOReturnSuccess)
    }

    func rfcommChannelData(_ rfcommChannel: IOBluetoothRFCOMMChannel!,
                           data dataPointer: UnsafeMutableRawPointer!,
                           length dataLength: Int) {
        guard dataLength > 0 else { return }
        buffer.append(dataPointer.assumingMemoryBound(to: UInt8.self), count: dataLength)
        if buffer.count > 256 * 1024 {
            buffer.removeFirst(buffer.count - 128 * 1024)
        }
    }

    func rfcommChannelClosed(_ rfcommChannel: IOBluetoothRFCOMMChannel!) {
        closed = true
        onClosed?()
    }
}
