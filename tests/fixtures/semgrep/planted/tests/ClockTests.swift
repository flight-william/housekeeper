import XCTest

final class ClockTests: XCTestCase {
    func testSleeps() async throws {
        Thread.sleep(forTimeInterval: 1)
        usleep(10)
        try await Task.sleep(nanoseconds: 1)
        let s = "Thread.sleep(forTimeInterval: 1) usleep(1) Date()" // Thread.sleep(forTimeInterval: 2) Date()
        XCTAssertFalse(s.isEmpty)
    }

    func testClock() {
        let now = Date()
        XCTAssertNotNil(now)
    }
}
