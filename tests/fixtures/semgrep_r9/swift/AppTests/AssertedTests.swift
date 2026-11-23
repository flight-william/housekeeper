import Foundation
import Testing
import XCTest

// A try? whose result is compared or asserted: the check is the test.
@Test func parsesOnlyGoodInput() throws {
    #expect((try? parse("bad")) == nil)
    #expect(try? parse("good") != nil)
    let value = try #require(try? parse("good"))
    #expect(value == 1)
}

final class ParseTests: XCTestCase {
    func testBadInputGivesNothing() {
        XCTAssertNil(try? parse("bad"))
        XCTAssertNotNil(try? parse("good"))
        let failed = (try? parse("bad")) == nil
        XCTAssertTrue(failed)
        if (try? parse("good")) != nil {
            XCTAssertTrue(true)
        }
    }
}

// Cleanup: nobody can act on a close or a remove that fails.
final class HandleTests: XCTestCase {
    func testWritesThenCleansUp() throws {
        let handle = try FileHandle(forWritingTo: URL(fileURLWithPath: NSTemporaryDirectory()))
        try? handle.close()
        try? store.remove(key: "k")
        try? store.removeFile(named: "k")
    }
}
