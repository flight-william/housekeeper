import Foundation
import Testing

// Removing a temp file or folder inline: nobody can act on the error.
@Test func removesItsFiles() {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("x")
    try? FileManager.default.removeItem(at: url)
    try? FileManager.default.removeItem(atPath: url.path)
    let files = FileManager.default
    try? files.removeItem(at: url)
}
