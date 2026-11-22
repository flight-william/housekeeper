import Testing

struct CleanTests {
    @Test("named", .tags(.fast))
    func ok() throws {
        let value = try load()
        _ = value
    }
}
