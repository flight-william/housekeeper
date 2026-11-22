import Testing
import XCTest

// swiftlint:disable force_cast
@Suite(.disabled("flaky on CI"))
struct DisabledSuite {
    @Test func one() {}
}

struct Traits {
    @Test("named", .tags(.slow), .disabled("later"))
    func two() {}

    @Test(
        .disabled(if: true)
    )
    func three() {}

    @Test func swallowed() {
        let value = try? load()
        _ = value
    }
}

final class OldTests: XCTestCase {
    func testSkip() throws {
        throw XCTSkip("not now")
    }

    func testSkipIf() throws {
        try XCTSkipIf(true, "never")
    }
}

// swiftformat:disable all
// periphery:ignore
final class KnownIssueTests: XCTestCase {
    func testExpected() {
        XCTExpectFailure("known bug")
        XCTAssertTrue(false)
    }
}

struct KnownIssues {
    @Test func known() {
        withKnownIssue {
            Issue.record("known bug")
        }
    }
}

// A try? on any other call still swallows the error, a file write among them.
@Test func writes() {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("y")
    try? Data().write(to: url)
    try? check(FileManager.default.removeItem(at: url))
}
