import AppKit
import Foundation
import ServiceManagement

@MainActor
final class AppModel: ObservableObject {
    @Published var snapshot = Snapshot(running: false)
    @Published var startError: String?
    @Published var notice: String?
    @Published var launchAtLogin = SMAppService.mainApp.status == .enabled
    @Published var section: Section = .overview
    @Published var selectedJobID: Job.ID?

    private var timer: Timer?
    private let dataDir = FileManager.default
        .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("Dotline", isDirectory: true)

    init() {
        start()
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }
    }

    func start(port: UInt16 = 0) {
        let result = Core.start(dataDir: dataDir, port: port)
        startError = result?.ok == true ? nil : (result?.error ?? String(localized: "Could not start"))
        refresh()
    }

    func refresh() {
        // lpstat runs off the main thread; the UI stays responsive.
        Task.detached {
            let next = Core.snapshot()
            await MainActor.run { if let next { self.snapshot = next } }
        }
    }

    var endpoint: String { "http://127.0.0.1:\(snapshot.port ?? 0)" }
    var jobs: [Job] { snapshot.jobs ?? [] }
    var printers: [Printer] { snapshot.printers ?? [] }
    var todayJobs: [Job] { jobs.filter { Calendar.current.isDateInToday($0.date) && !$0.isTest } }
    var failedToday: Int { todayJobs.filter(\.failed).count }
    var sentToday: Int { todayJobs.count - failedToday }
    var readyPrinters: Int { printers.filter { $0.state != "disabled" }.count }

    func show(job: Job) {
        selectedJobID = job.id
        section = .jobs
    }

    func copy(_ text: String, notice: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        flash(notice)
    }

    func rotateToken() {
        let result = Core.rotateToken()
        flash(result?.ok == true
              ? String(localized: "New token issued. Update it in every connected app.")
              : (result?.error ?? String(localized: "Error")))
        refresh()
    }

    func testPrint(_ printer: Printer, language: String) {
        Task.detached {
            let result = Core.testPrint(printer: printer.name, language: language)
            await MainActor.run {
                self.flash(result?.state == "sent"
                           ? String(localized: "Test label sent to \(printer.name)")
                           : String(localized: "Test print failed: \(result?.detail ?? "?")"))
                self.refresh()
            }
        }
    }

    func clearJobs() {
        Core.clearJobs()
        selectedJobID = nil
        refresh()
    }

    func setLaunchAtLogin(_ on: Bool) {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
        } catch {
            flash(String(localized: "Could not change: \(error.localizedDescription)"))
        }
        launchAtLogin = SMAppService.mainApp.status == .enabled
    }

    private func flash(_ text: String) {
        notice = text
        Task {
            try? await Task.sleep(for: .seconds(4))
            if notice == text { notice = nil }
        }
    }
}
