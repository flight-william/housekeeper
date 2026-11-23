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

    // An expectation's timeout only bounds a failure: a test that passes never waits it out.
    func testWaitsOnAnExpectationWithATimeout() async {
        let done = expectation(description: "done")
        work.run { done.fulfill() }
        wait(for: [done], timeout: 5)
        self.wait(for: [done], timeout: 5)
        await fulfillment(of: [done], timeout: 5)
        _ = semaphore.wait(timeout: deadline)
    }

    func testAwaitsWithoutTimeout() async {
        let value = await compute()
        XCTAssertEqual(value, 1)
    }

    // A made-up user's path handed to a formatter and compared: data, never read.
    func testShortensAMadeUpHomePath() {
        XCTAssertEqual(shorten("/Users/tester/Desktop/Shop"), "~/Desktop/Shop")
        XCTAssertEqual(URL(fileURLWithPath: "/Users/someone/project").lastPathComponent, "project")
        print("/home/someone/project")
    }

    func testAssertsOnTheStandInLog() {
        let log = ""
        XCTAssertEqual(log, "find-generic-password -s svc")
        XCTAssertTrue(log.hasPrefix("/usr/bin/security"))
        XCTAssertFalse(
            log.contains("/usr/bin/security")
        )
    }
}

final class XCTestWaitFormsWithALimitTests: XCTestCase {
    func testWaitsWithALimitInXCTestsOwnForms() async {
        let done = expectation(description: "done")
        await self.fulfillment(of: [done], timeout: 5)
        await XCTWaiter().fulfillment(of: [done], timeout: 5, enforceOrder: true)
        wait(for: [done], timeout: 5, enforceOrder: true)
        waitForExpectations(timeout: 5)
        _ = semaphore.wait(wallTimeout: wallDeadline)
    }
}
