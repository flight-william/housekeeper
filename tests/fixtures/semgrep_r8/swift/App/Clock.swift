import Foundation

struct Clock {
    func now() -> Date {
        Thread.sleep(forTimeInterval: 0)
        _ = Locale.current
        _ = URLSession.shared
        _ = NSHomeDirectory()
        return Date()
    }
}
