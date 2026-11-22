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
        wait(for: [done], timeout: 1)
        self.wait(for: [done], timeout: 1)
        await fulfillment(of: [done], timeout: 1)
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

    func testPaths() {
        let a = "/Users/someone/project"
        let b = "/home/someone/project"
        let c = "~/project"
        XCTAssertNotNil([a, b, c])
    }

    func testKeychainCallInsideAnAssert() {
        XCTAssertEqual(SecItemCopyMatching(query, &result), errSecSuccess)
    }
}
