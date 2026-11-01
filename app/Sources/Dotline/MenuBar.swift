import SwiftUI

/// Compact popover from the menu bar icon.
struct MenuBarView: View {
    @EnvironmentObject var model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                AppIcon(size: 32)
                VStack(alignment: .leading, spacing: 1) {
                    Text(verbatim: "Dotline").font(.headline)
                    HStack(spacing: 5) {
                        Circle().fill(model.snapshot.running ? Color.green : Color.red).frame(width: 6, height: 6)
                        if model.snapshot.running {
                            Text("Running on port \(String(model.snapshot.port ?? 0))")
                                .font(.caption).foregroundStyle(.secondary)
                        } else {
                            Text(model.startError ?? String(localized: "Stopped"))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                Spacer()
                Button {
                    model.copy(model.snapshot.token ?? "", notice: String(localized: "Access token copied"))
                } label: { Image(systemName: "key") }
                    .help("Copy access token")
                    .buttonStyle(.borderless)
            }

            HStack(spacing: 8) {
                mini("\(model.sentToday)", String(localized: "Sent"), .blue)
                mini("\(model.failedToday)", String(localized: "Failed"), model.failedToday > 0 ? .red : .secondary)
                mini("\(model.readyPrinters)/\(model.printers.count)", String(localized: "Printers"), .green)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text("Recent").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                if model.jobs.isEmpty {
                    Text("No jobs yet.").font(.caption).foregroundStyle(.secondary).padding(.vertical, 6)
                }
                ForEach(model.jobs.prefix(5)) { job in
                    Button {
                        model.show(job: job)
                        openDashboard()
                    } label: { JobRow(job: job) }
                        .buttonStyle(.plain)
                }
            }

            if let notice = model.notice {
                Text(notice).font(.caption).foregroundStyle(.secondary)
            }
            Divider()
            HStack {
                Toggle("Open at login", isOn: Binding(get: { model.launchAtLogin }, set: model.setLaunchAtLogin))
                    .toggleStyle(.checkbox).font(.caption)
                Spacer()
                Button("Dashboard") { openDashboard() }.buttonStyle(.borderedProminent).controlSize(.small)
                Button("Quit") { NSApplication.shared.terminate(nil) }.controlSize(.small)
            }
        }
        .padding(14)
        .frame(width: 360)
        .onAppear { model.refresh() }
    }

    private func openDashboard() {
        openWindow(id: "dashboard")
        NSApp.activate(ignoringOtherApps: true)
    }

    private func mini(_ value: String, _ title: String, _ tint: Color) -> some View {
        VStack(spacing: 2) {
            Text(value).font(.title3.weight(.semibold)).monospacedDigit().foregroundStyle(tint)
            Text(title).font(.caption2).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

/// The bundled app icon, so in-app branding always matches the Dock.
struct AppIcon: View {
    var size: CGFloat
    var body: some View {
        Image(nsImage: NSApp.applicationIconImage)
            .resizable()
            .frame(width: size * 1.22, height: size * 1.22) // icon art has a transparent margin
            .frame(width: size, height: size)
    }
}

/// Menu bar glyph: printer slot over a row of dots, the last one still printing.
enum MenuBarGlyph {
    static func image(active: Bool) -> NSImage {
        let image = NSImage(size: NSSize(width: 20, height: 16), flipped: true) { _ in
            NSColor.black.setFill()
            NSBezierPath(roundedRect: NSRect(x: 1, y: 1, width: 18, height: 4), xRadius: 2, yRadius: 2).fill()
            NSBezierPath(roundedRect: NSRect(x: 3.5, y: 3, width: 13, height: 12), xRadius: 2, yRadius: 2).fill()
            NSGraphicsContext.current?.compositingOperation = .clear
            for i in 0..<3 {
                let dot = NSRect(x: 5.25 + CGFloat(i) * 3.5, y: 7.5, width: 2.5, height: 2.5)
                if active || i < 2 { NSBezierPath(ovalIn: dot).fill() }
            }
            return true
        }
        image.isTemplate = true
        return image
    }
}
