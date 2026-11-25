import IOBluetooth

enum DeviceIdentity {
    static func isBose(name: String, vendorID: Int?, vendorSource: Int?) -> Bool {
        // Device ID source 1 uses Bluetooth SIG company identifiers, not USB IDs.
        if vendorSource == 1, vendorID == 0x009E { return true }
        let lower = name.lowercased()
        return lower.contains("bose")
            || lower.contains("quietcomfort")
            || lower.contains("qc35")
            || lower.contains("qc45")
            || lower.contains("qc ultra")
    }

    static func isBose(_ device: IOBluetoothDevice) -> Bool {
        // Attribute IDs are service-specific: only inspect the PnP Information
        // service (0x1200), rather than unrelated services with the same IDs.
        let record = device.getServiceRecord(for: IOBluetoothSDPUUID(uuid16: 0x1200))
        return isBose(
            name: device.name ?? "",
            vendorID: record?.getAttributeDataElement(0x0201)?.getNumberValue()?.intValue,
            vendorSource: record?.getAttributeDataElement(0x0205)?.getNumberValue()?.intValue
        )
    }
}
