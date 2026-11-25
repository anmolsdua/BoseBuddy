import Foundation

@main
enum DeviceIdentityTests {
    static func main() {
        assert(DeviceIdentity.isBose(name: "Obsidian", vendorID: 0x009E, vendorSource: 1))
        assert(DeviceIdentity.isBose(name: "", vendorID: 0x009E, vendorSource: 1))
        assert(!DeviceIdentity.isBose(name: "Other headphones", vendorID: 0x009E, vendorSource: 2))
        assert(!DeviceIdentity.isBose(name: "Other headphones", vendorID: 0x009E, vendorSource: nil))
        assert(!DeviceIdentity.isBose(name: "Other headphones", vendorID: 0x004C, vendorSource: 1))
        assert(!DeviceIdentity.isBose(name: "Obsidian", vendorID: nil, vendorSource: nil))
        for name in ["Bose QC Headphones", "QuietComfort", "QC35", "QC45", "QC Ultra"] {
            assert(DeviceIdentity.isBose(name: name, vendorID: nil, vendorSource: nil))
        }
        print("Device identity tests passed")
    }
}
