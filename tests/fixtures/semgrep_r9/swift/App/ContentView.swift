import SwiftUI

struct ContentView: View {
    @State private var ok = false

    var body: some View {
        VStack {
            Button("Save") {}
                .disabled(!ok)
            Toggle("On", isOn: $ok)
                .disabled(ok == false)
        }
    }

    func load() -> Int? {
        try? decode()
    }
}
