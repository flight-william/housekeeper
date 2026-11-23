import Foundation
import Testing
import XCTest

// Each shape in which a try? result is checked, once without and once with
// parentheses around the try? expression: none is R9 (patterns/r9.yml lists them).
final class CheckedTests: XCTestCase {
    func testEveryCheckedShape() throws {
        XCTAssertNil(try? parse("a1"))
        XCTAssertNil((try? parse("a2")))
        XCTAssertNotNil(try? parse("b1"))
        XCTAssertNotNil((try? parse("b2")))
        _ = try XCTUnwrap(try? parse("c1"))
        _ = try XCTUnwrap((try? parse("c2")))
        _ = try #require(try? parse("d1"))
        _ = try #require((try? parse("d2")))
        XCTAssertEqual(try? parse("e1"), 1)
        XCTAssertEqual((try? parse("e2")), 1)
        XCTAssertEqual(1, try? parse("f1"))
        XCTAssertEqual(1, (try? parse("f2")))
        XCTAssertNotEqual(try? parse("g1"), 2)
        XCTAssertNotEqual((try? parse("g2")), 2)
        XCTAssertNotEqual(2, try? parse("h1"))
        XCTAssertNotEqual(2, (try? parse("h2")))
        let i1 = (try? parse("i1")) != nil
        let i2 = ((try? parse("i2"))) != nil
        let j1 = (try? parse("j1")) == 3
        let j2 = ((try? parse("j2"))) == 3
        let k1 = 3 == (try? parse("k1"))
        let k2 = nil != ((try? parse("k2")))
        #expect((try? parse("l1")) == nil)
        #expect(((try? parse("l2"))) == nil)
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
