import Foundation

@main
enum RequestDescriptionTests {
    static func main() {
        // The QC45 rejects the charging-state read.
        assert(RequestDescription.deviceError(fblock: 2, function: 3, op: .get, code: 4)
               == "Headphones don't support reading charging state [2.3 get, error 4 unsupported function]")
        assert(RequestDescription.deviceError(fblock: 1, function: 7, op: .setGet, code: 6)
               == "Headphones rejected the value when changing EQ [1.7 setGet, error 6 invalid data]")
        assert(RequestDescription.failure(fblock: 31, function: 1, op: .start,
                                          error: BmapError.timeout, seconds: 2.84)
               == "No reply from the headphones when requesting mode list after 2.8s [31.1 start]")
        // Unknown requests still read sensibly.
        assert(RequestDescription.action(fblock: 9, function: 9, op: .get) == "reading function 9.9")
        print("Request description tests passed")
    }
}
