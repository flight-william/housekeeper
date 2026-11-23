import Testing
import XCTest

// A try? that no assertion inspects still swallows the error.
final class SwallowTests: XCTestCase {
    func testSave() {
        XCTAssertNoThrow(try? save())
        XCTAssertEqual(1, 1, "result: \(try? save())")
        XCTAssertNil(try? parse("bad"), "also \(try? save())")
    }
}

@Test func checksSomethingElse() {
    #expect(check(try? load()))
}
