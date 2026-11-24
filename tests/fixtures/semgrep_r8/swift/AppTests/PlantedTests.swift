import Foundation
import Network
import Security
import XCTest

final class PlantedTests: XCTestCase {
    func testSleep() async throws {
        Thread.sleep(forTimeInterval: 1)
        usleep(10)
        sleep(1)
        try await Task.sleep(nanoseconds: 1)
        DispatchQueue.main.asyncAfter(deadline: deadline) {}
    }

    func testWait() async {
        let done = expectation(description: "done")
        semaphore.wait()
        wait(for: [done], timeout: .infinity)
        await fulfillment(of: [done])
    }

    func testClock() {
        _ = Date()
        _ = Date.now
        schedule(at: .now)
        _ = Date(timeIntervalSinceNow: 5)
        _ = ContinuousClock.now
        _ = SuspendingClock.now
        _ = ContinuousClock()
        _ = SuspendingClock()
        _ = DispatchTime.now()
        _ = DispatchWallTime.now()
        _ = CFAbsoluteTimeGetCurrent()
    }

    func testRandom() {
        _ = Int.random(in: 0..<6)
        _ = Bool.random()
        _ = [1, 2].randomElement()
        _ = [1, 2].shuffled()
        var list = [1, 2]
        list.shuffle()
    }

    func testLocale() {
        _ = Locale.current
        _ = Locale.autoupdatingCurrent
        _ = Locale.preferredLanguages
        _ = TimeZone.current
        _ = TimeZone.autoupdatingCurrent
        _ = Calendar.current
        _ = Calendar.autoupdatingCurrent
    }

    func testHome() {
        _ = NSHomeDirectory()
        _ = NSHomeDirectoryForUser("someone")
        _ = FileManager.default.homeDirectoryForCurrentUser
        _ = FileManager.default.homeDirectory(forUser: "someone")
        _ = URL.homeDirectory
        _ = ("~" as NSString).expandingTildeInPath
        _ = ProcessInfo.processInfo.environment["HOME"]
        _ = getenv("HOME")
    }

    func testKeychain() {
        _ = SecItemCopyMatching(query, &result)
        _ = SecItemAdd(query, nil)
        _ = SecItemUpdate(query, changes)
        _ = SecItemDelete(query)
        let tool = "/usr/bin/security"
        let args = ["find-generic-password", "-s", "x"]
        XCTAssertNotNil([tool, args] as [Any])
    }

    func testNetwork() {
        _ = URLSession.shared
        _ = URLSession(configuration: .default)
        _ = URLSession(configuration: .default, delegate: nil, delegateQueue: nil)
        _ = NWConnection(host: "example.invalid", port: 443, using: .tcp)
    }

    func testPaths() throws {
        let a = FileManager.default.fileExists(atPath: "/Users/someone/project")
        let b = try String(contentsOfFile: "/home/someone/project/notes.txt", encoding: .utf8)
        let c = try Data(contentsOf: URL(fileURLWithPath: "~/project/data.bin"))
        XCTAssertNotNil([a, b, c] as [Any])
    }

    func testKeychainCallInsideAnAssert() {
        XCTAssertEqual(SecItemCopyMatching(query, &result), errSecSuccess)
    }
}

final class MoveTests: XCTestCase {
    func testMovesToAMachinePath() throws {
        try FileManager.default.copyItem(atPath: "tmp", toPath: "/Users/someone/a")
        try FileManager.default.moveItem(atPath: "tmp", toPath: "/Users/someone/b")
        let from = URL(fileURLWithPath: "tmp")
        try FileManager.default.copyItem(at: from, to: URL(fileURLWithPath: "/home/someone/c"))
        try FileManager.default.moveItem(at: from, to: URL(fileURLWithPath: "~/d"))
    }
}

final class UnboundedWaitTests: XCTestCase {
    func testWaitsWithNoLimit() async {
        let done = expectation(description: "done")
        group.wait()
        self.wait(for: [done], timeout: .infinity)
        await fulfillment(of: [done], timeout: .infinity)
        _ = semaphore.wait(timeout: .distantFuture)
        await fulfillment(of: [done], enforceOrder: true)
    }

    func testSleepsThenWaits() {
        let done = expectation(description: "done")
        Thread.sleep(forTimeInterval: 1)
        wait(for: [done], timeout: 1)
    }
}

final class XCTestWaitFormsTests: XCTestCase {
    func testWaitsWithNoLimitInXCTestsOwnForms() async {
        let done = expectation(description: "done")
        await self.fulfillment(of: [done])
        await XCTWaiter().fulfillment(of: [done])
        await XCTWaiter.fulfillment(of: [done], timeout: .infinity)
        wait(for: [done], timeout: .infinity, enforceOrder: true)
        waitForExpectations(timeout: .infinity)
        wait(for: [done], timeout: TimeInterval.infinity)
        wait(for: [done], timeout: Double.infinity)
        wait(for: [done], timeout: .greatestFiniteMagnitude)
        _ = semaphore.wait(wallTimeout: .distantFuture)
    }
}
