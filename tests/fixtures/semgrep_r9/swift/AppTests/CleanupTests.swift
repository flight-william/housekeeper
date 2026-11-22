import Foundation
import XCTest

final class CleanupTests: XCTestCase {
    var url: URL!

    override func tearDown() {
        try? FileManager.default.removeItem(at: url)
        super.tearDown()
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: url)
    }

    func testCleansUp() throws {
        defer { try? FileManager.default.removeItem(at: url) }
        defer {
            try? FileManager.default.removeItem(at: url)
        }
        let text = "try? decode() // swiftlint:disable all"
        let trait = "@Test(.disabled())"
        // try? is fine here in a comment
        let tools = "// swiftformat:disable // periphery:ignore"
        let markers = "XCTSkip(\"later\") XCTExpectFailure withKnownIssue"
        XCTAssertFalse(tools.isEmpty || markers.isEmpty)
        XCTAssertFalse(text.isEmpty || trait.isEmpty)
    }
}

final class Holder {
    let url: URL

    init(url: URL) {
        self.url = url
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }
}
