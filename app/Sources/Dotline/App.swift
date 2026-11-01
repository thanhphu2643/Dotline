import SwiftUI

@main
struct DotlineApp: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        // Listed first so it opens at launch; the menu bar icon stays after it closes.
        Window("Dotline", id: "dashboard") {
            DashboardView().environmentObject(model)
                .onAppear { NSApp.activate(ignoringOtherApps: true) }
        }
        .defaultSize(width: 1100, height: 720)

        MenuBarExtra {
            MenuBarView().environmentObject(model)
        } label: {
            Image(nsImage: MenuBarGlyph.image(active: model.snapshot.running))
        }
        .menuBarExtraStyle(.window)
    }
}
