import Testing
import XCTest

// Each shape in which a try? result is not checked, once without and once with
// parentheses: every one swallows the error and is R9 (patterns/r9.yml lists them).
final class SwallowTests: XCTestCase {
    func testEveryUncheckedShape() throws {
        XCTAssertNoThrow(try? save("m1"))
        XCTAssertNoThrow((try? save("m2")))
        XCTAssertEqual(1, 1, "result: \(try? save("n1"))")
        XCTAssertNil(try? parse("n2"), "also \((try? save("n2")))")
        try? save("o1")
        _ = (try? save("o2"))
        XCTAssertEqual(count(try? load("p1")), 1)
        let p2 = count((try? load("p2"))) == 1
        #expect(check(try? load("q1")))
        #expect(check((try? load("q2"))))
    }
}
