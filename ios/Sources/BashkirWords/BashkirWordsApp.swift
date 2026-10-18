import SwiftUI

@main
struct BashkirWordsApp: App {
    var body: some Scene {
        WindowGroup {
            GameWebView()
                .ignoresSafeArea()
                .statusBarHidden()
                .preferredColorScheme(.light)
        }
    }
}
