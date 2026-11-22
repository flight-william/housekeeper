import Foundation
import XCTest

final class CleanTests: XCTestCase {
    func testStringsAndCommentsAreNotCalls() {
        let note = "Thread.sleep(1) Date() Locale.current NSHomeDirectory() URLSession.shared"
        // Thread.sleep(forTimeInterval: 1) Date.now TimeZone.current wait(for: [e], timeout: 1)
        XCTAssertFalse(note.isEmpty)
    }

    func testFixedValues() {
        let date = Date(timeIntervalSince1970: 0)
        let zone = TimeZone(identifier: "UTC")
        let locale = Locale(identifier: "en_US_POSIX")
        var generator = SeededGenerator(seed: 4)
        let roll = Int.random(in: 0..<6, using: &generator)
        let fake = fakeClock.now
        XCTAssertNotNil([date, zone, locale, roll, fake] as [Any?])
    }

    func testStubbedSession() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubProtocol.self]
        let session = URLSession(configuration: configuration)
        XCTAssertNotNil(session)
    }

    func testTempFolder() {
        let folder = FileManager.default.temporaryDirectory
        let message = "files live under /Users/ on a Mac"
        let tool = "/usr/bin/security-helper"
        XCTAssertNotNil([folder, message, tool] as [Any])
    }

    func testAwaitsWithoutTimeout() async {
        let value = await compute()
        XCTAssertEqual(value, 1)
    }
}
