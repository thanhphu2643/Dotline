import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct DashboardView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        NavigationSplitView {
            List(Section.allCases, selection: Binding(get: { model.section }, set: { if let s = $0 { model.section = s } })) { section in
                Label(section.title, systemImage: section.icon)
                    .badge(section == .jobs && model.failedToday > 0 ? model.failedToday : 0)
                    .tag(section)
            }
            .navigationSplitViewColumnWidth(min: 190, ideal: 210)
            .safeAreaInset(edge: .bottom) { statusPill.padding(12) }
        } detail: {
            Group {
                switch model.section {
                case .overview: OverviewView()
                case .jobs: JobsView()
                case .printers: PrintersView()
                case .pairing: ConnectView()
                case .inspector: InspectorView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: .windowBackgroundColor))
            .navigationTitle(model.section.title)
            .toolbar {
                ToolbarItem {
                    Button { model.refresh() } label: { Label("Refresh", systemImage: "arrow.clockwise") }
                        .help("Refresh")
                }
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .overlay(alignment: .bottom) {
            if let notice = model.notice {
                Text(notice)
                    .font(.callout)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(.regularMaterial, in: Capsule())
                    .shadow(radius: 6, y: 2)
                    .padding(.bottom, 18)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.spring(duration: 0.3), value: model.notice)
    }

    private var statusPill: some View {
        HStack(spacing: 8) {
            Circle().fill(model.snapshot.running ? Color.green : Color.red).frame(width: 8, height: 8)
                .shadow(color: model.snapshot.running ? .green : .red, radius: 3)
            VStack(alignment: .leading, spacing: 0) {
                Text(model.snapshot.running ? "Running" : "Stopped").font(.caption.weight(.semibold))
                Text(verbatim: model.snapshot.running ? "127.0.0.1:\(model.snapshot.port ?? 0)" : (model.startError ?? ""))
                    .font(.caption2.monospaced()).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer()
        }
        .padding(10)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

// MARK: - Overview

struct OverviewView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                hero
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 4), spacing: 12) {
                    StatCard(title: String(localized: "Sent today"), value: "\(model.sentToday)",
                             systemImage: "tag.fill", tint: .blue)
                    StatCard(title: String(localized: "Failed today"), value: "\(model.failedToday)",
                             systemImage: "exclamationmark.triangle.fill", tint: model.failedToday > 0 ? .red : .secondary)
                    StatCard(title: String(localized: "Printers ready"), value: "\(model.readyPrinters)/\(model.printers.count)",
                             systemImage: "printer.fill", tint: .green)
                    StatCard(title: String(localized: "Data sent today"), value: formatBytes(model.todayJobs.reduce(0) { $0 + $1.bytes }),
                             systemImage: "arrow.up.doc.fill", tint: .purple)
                }
                HStack(alignment: .top, spacing: 16) {
                    Card(title: String(localized: "Latest label"), systemImage: "eye") {
                        if let job = model.jobs.first(where: { !$0.failed }) {
                            JobPreviewLoader(job: job, maxHeight: 220)
                            HStack {
                                Text(verbatim: job.id).font(.caption.monospaced()).lineLimit(1)
                                Spacer()
                                Text(job.date, format: .relative(presentation: .named)).font(.caption).foregroundStyle(.secondary)
                            }
                        } else {
                            ContentUnavailableView("No Labels Yet", systemImage: "tag",
                                                   description: Text("Labels sent by your web apps appear here."))
                                .frame(height: 260)
                        }
                    }
                    .frame(maxWidth: .infinity)
                    Card(title: String(localized: "Recent activity"), systemImage: "clock") {
                        if model.jobs.isEmpty {
                            Text("No jobs yet.").foregroundStyle(.secondary).frame(height: 260)
                        }
                        VStack(spacing: 0) {
                            ForEach(model.jobs.prefix(7)) { job in
                                Button { model.show(job: job) } label: { JobRow(job: job) }
                                    .buttonStyle(.plain)
                                if job.id != model.jobs.prefix(7).last?.id { Divider() }
                            }
                        }
                        if model.jobs.count > 7 {
                            Button("View all \(model.jobs.count) jobs →") { model.section = .jobs }
                                .buttonStyle(.link)
                        }
                    }
                    .frame(width: 340)
                }
            }
            .padding(24)
        }
    }

    private var hero: some View {
        HStack(spacing: 16) {
            AppIcon(size: 60)
            VStack(alignment: .leading, spacing: 4) {
                Text(verbatim: "Dotline").font(.title2.bold())
                Text("Raw label printing for the web").font(.callout).foregroundStyle(.secondary)
                HStack(spacing: 6) {
                    StateBadge(text: model.snapshot.running ? String(localized: "Accepting jobs") : String(localized: "Stopped"),
                               color: model.snapshot.running ? .green : .red)
                    Text(verbatim: model.endpoint).font(.callout.monospaced()).foregroundStyle(.secondary)
                }
            }
            Spacer()
            if model.snapshot.running {
                Button { model.copy(model.endpoint, notice: String(localized: "Address copied")) } label: {
                    Label("Copy address", systemImage: "doc.on.doc")
                }
                Button { model.section = .pairing } label: { Label("Connect an app", systemImage: "link") }
                    .buttonStyle(.borderedProminent)
            } else {
                Button("Restart") { model.start() }.buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .background(
            LinearGradient(colors: [.blue.opacity(0.10), .indigo.opacity(0.04)], startPoint: .leading, endPoint: .trailing),
            in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.blue.opacity(0.15)))
    }
}

struct JobRow: View {
    let job: Job
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: job.failed ? "xmark.octagon.fill" : (job.isTest ? "testtube.2" : "checkmark.circle.fill"))
                .foregroundStyle(job.failed ? .red : (job.isTest ? .orange : .green))
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(job.isTest ? String(localized: "Test label") : job.id).font(.callout.monospaced()).lineLimit(1)
                Text(job.failed ? (job.detail ?? String(localized: "Failed")) : job.printer)
                    .font(.caption).foregroundStyle(job.failed ? .red : .secondary).lineLimit(1)
            }
            Spacer()
            Text(job.date, style: .time).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
        }
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}

/// Loads a job's preview off the main thread and keeps it while visible.
struct JobPreviewLoader: View {
    let job: Job
    var maxHeight: CGFloat = 320
    @State private var preview: LabelPreview?
    @State private var loaded = false

    var body: some View {
        ZStack {
            if loaded {
                LabelPreviewView(preview: preview, maxHeight: maxHeight)
            } else {
                ProgressView().frame(height: maxHeight)
            }
        }
        .task(id: job.id + (job.payloadHash ?? "")) {
            loaded = false
            let id = job.id
            preview = await Task.detached { Core.jobPreview(id: id) }.value
            loaded = true
        }
    }
}

// MARK: - Jobs

struct JobsView: View {
    @EnvironmentObject var model: AppModel
    @State private var search = ""
    @State private var filter = "all"

    private var filtered: [Job] {
        model.jobs.filter { job in
            let state = filter == "all" || (filter == "failed" ? job.failed : !job.failed)
            let text = search.isEmpty || job.id.localizedCaseInsensitiveContains(search)
                || job.printer.localizedCaseInsensitiveContains(search)
            return state && text
        }
    }

    var body: some View {
        HSplitView {
            VStack(spacing: 0) {
                VStack(spacing: 8) {
                    TextField("Search job ID or printer…", text: $search).textFieldStyle(.roundedBorder)
                    Picker("", selection: $filter) {
                        Text("All").tag("all")
                        Text("Sent").tag("sent")
                        Text("Failed").tag("failed")
                    }
                    .pickerStyle(.segmented).labelsHidden()
                }
                .padding(12)
                Divider()
                if filtered.isEmpty {
                    ContentUnavailableView("No Jobs", systemImage: "tray",
                                           description: Text(model.jobs.isEmpty
                                                             ? String(localized: "Jobs from your web apps appear here.")
                                                             : String(localized: "Try clearing the filter.")))
                        .frame(maxHeight: .infinity)
                } else {
                    List(filtered, selection: $model.selectedJobID) { job in
                        JobRow(job: job).tag(job.id)
                    }
                    .listStyle(.inset)
                }
                Divider()
                HStack {
                    Text("\(filtered.count) jobs").font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button("Clear history", role: .destructive) { model.clearJobs() }
                        .buttonStyle(.borderless).font(.caption)
                        .disabled(model.jobs.isEmpty)
                }
                .padding(10)
            }
            .frame(minWidth: 280, idealWidth: 320, maxWidth: 420)

            Group {
                if let job = model.jobs.first(where: { $0.id == model.selectedJobID }) {
                    JobDetailView(job: job)
                } else {
                    ContentUnavailableView("Select a Job", systemImage: "hand.point.left",
                                           description: Text("Preview the label exactly as its bytes reached the printer."))
                }
            }
            .frame(minWidth: 420, maxWidth: .infinity, maxHeight: .infinity)
        }
        .onAppear {
            if model.selectedJobID == nil { model.selectedJobID = model.jobs.first?.id }
        }
    }
}

struct JobDetailView: View {
    @EnvironmentObject var model: AppModel
    let job: Job

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(job.isTest ? "Test label" : "Job").font(.caption).foregroundStyle(.secondary)
                        Text(verbatim: job.id).font(.title3.monospaced().weight(.semibold)).textSelection(.enabled)
                    }
                    Spacer()
                    StateBadge(text: job.stateText, color: job.stateColor)
                }
                if job.failed, let detail = job.detail {
                    Label(detail, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.red.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
                }
                Card(title: String(localized: "Label preview"), systemImage: "eye") {
                    JobPreviewLoader(job: job, maxHeight: 300)
                }
                Card(title: String(localized: "Details"), systemImage: "info.circle") {
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 16, verticalSpacing: 8) {
                        row("Printer", job.printer)
                        row("Time", job.date.formatted(date: .abbreviated, time: .standard))
                        row("CUPS request", job.requestId ?? "—")
                        row("Size", formatBytes(job.bytes))
                        row("SHA-256", job.payloadHash ?? "—", mono: true)
                    }
                    HStack {
                        Button { model.copy(job.id, notice: String(localized: "Job ID copied")) } label: {
                            Label("Copy job ID", systemImage: "doc.on.doc")
                        }
                        if let hash = job.payloadHash {
                            Button { model.copy(hash, notice: String(localized: "Hash copied")) } label: {
                                Label("Copy hash", systemImage: "number")
                            }
                        }
                    }
                    .padding(.top, 4)
                }
            }
            .padding(24)
        }
    }

    private func row(_ title: LocalizedStringKey, _ value: String, mono: Bool = false) -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            Text(verbatim: value).font(mono ? .caption.monospaced() : .body).textSelection(.enabled).lineLimit(2)
        }
    }
}

// MARK: - Printers

struct PrintersView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let error = model.snapshot.printersError {
                    Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 14)], spacing: 14) {
                    ForEach(model.printers) { printer in PrinterCard(printer: printer) }
                    networkCard
                }
                if model.printers.isEmpty {
                    ContentUnavailableView("No CUPS Printers", systemImage: "printer.dotmatrix",
                                           description: Text("Add your label printer in System Settings → Printers & Scanners."))
                }
            }
            .padding(24)
        }
        .toolbar {
            ToolbarItem {
                Button {
                    NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.Print-Scan-Settings.extension")!)
                } label: { Label("Add printer", systemImage: "plus") }
                    .help("Open macOS printer settings")
            }
        }
    }

    private var networkCard: some View {
        Card(title: String(localized: "Network printer (no CUPS)"), systemImage: "network") {
            Text("Apps can send straight to a network label printer on raw port 9100. Use a printer name like:")
                .font(.callout).foregroundStyle(.secondary)
            HStack {
                Text(verbatim: "tcp://192.168.1.50:9100").font(.callout.monospaced())
                    .padding(8).background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
                Button { model.copy("tcp://192.168.1.50:9100", notice: String(localized: "Example copied")) } label: {
                    Image(systemName: "doc.on.doc")
                }
                .buttonStyle(.borderless)
            }
        }
    }
}

struct PrinterCard: View {
    @EnvironmentObject var model: AppModel
    let printer: Printer

    private var jobs: [Job] { model.jobs.filter { $0.printer == printer.name } }

    var body: some View {
        Card {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "printer.fill")
                    .font(.title2)
                    .foregroundStyle(printer.stateColor)
                    .frame(width: 46, height: 46)
                    .background(printer.stateColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                VStack(alignment: .leading, spacing: 5) {
                    Text(verbatim: printer.name.replacingOccurrences(of: "_", with: " ")).font(.headline).lineLimit(2)
                    HStack(spacing: 6) {
                        StateBadge(text: printer.stateText, color: printer.stateColor)
                        if printer.isDefault { StateBadge(text: String(localized: "Default"), color: .secondary) }
                    }
                }
                Spacer(minLength: 0)
            }
            Text(verbatim: printer.name).font(.caption.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
            Divider()
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(jobs.filter { !$0.failed }.count) labels sent").font(.callout)
                    if let last = jobs.first {
                        Text("Last \(last.date.formatted(.relative(presentation: .named)))")
                            .font(.caption).foregroundStyle(.secondary)
                    } else {
                        Text("Nothing printed through Dotline yet").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Spacer()
                Menu {
                    Button { model.testPrint(printer, language: "zpl") } label: { Text(verbatim: "ZPL (Zebra)") }
                    Button { model.testPrint(printer, language: "ezpl") } label: { Text(verbatim: "EZPL (GoDEX)") }
                    Button { model.testPrint(printer, language: "tspl") } label: { Text(verbatim: "TSPL (TSC)") }
                } label: { Label("Test print", systemImage: "printer.dotmatrix") }
                    .fixedSize()
                Button { model.copy(printer.name, notice: String(localized: "Copied \(printer.name)")) } label: {
                    Image(systemName: "doc.on.doc")
                }
                .help("Copy queue name")
            }
        }
    }
}

// MARK: - Connect

struct ConnectView: View {
    @EnvironmentObject var model: AppModel
    @State private var reveal = false
    @State private var confirmRotate = false

    private var token: String { model.snapshot.token ?? "" }
    private var shownToken: String { reveal ? token : "<token>" }

    private var fetchSnippet: String {
        """
        await fetch("\(model.endpoint)/jobs", {
          method: "POST",
          headers: {
            Authorization: "Bearer \(shownToken)",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            id: "order-1042-label-1",   // idempotent: retries never reprint
            printer: "Zebra_ZD421",     // or "tcp://192.168.1.50:9100"
            payloadBase64: btoa("^XA^FO40,40^A0N,40^FDHello^FS^XZ"),
          }),
        });
        """
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Card(title: String(localized: "Access token"), systemImage: "key.fill") {
                    Text("Paste this token into the web app that prints labels. Anyone with it can send jobs to this Mac.")
                        .font(.callout).foregroundStyle(.secondary)
                    HStack(spacing: 10) {
                        Text(verbatim: reveal ? token : String(repeating: "•", count: 48))
                            .font(.system(.body, design: .monospaced))
                            .textSelection(.enabled)
                            .lineLimit(1)
                            .padding(10)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
                        Button { reveal.toggle() } label: { Image(systemName: reveal ? "eye.slash" : "eye") }
                            .help(reveal ? "Hide" : "Show")
                        Button { model.copy(token, notice: String(localized: "Access token copied")) } label: {
                            Label("Copy", systemImage: "doc.on.doc")
                        }
                        .buttonStyle(.borderedProminent)
                    }
                    HStack {
                        Spacer()
                        Button("New token…", role: .destructive) { confirmRotate = true }
                    }
                }
                Card(title: String(localized: "Address"), systemImage: "network") {
                    HStack {
                        Text(verbatim: model.endpoint).font(.body.monospaced()).textSelection(.enabled)
                        Spacer()
                        Button { model.copy(model.endpoint, notice: String(localized: "Address copied")) } label: {
                            Label("Copy", systemImage: "doc.on.doc")
                        }
                    }
                    Text("Only this Mac can connect (127.0.0.1). HTTPS pages can call it thanks to CORS and Private Network Access.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Card(title: String(localized: "Send a label from JavaScript"), systemImage: "curlybraces") {
                    snippet(fetchSnippet, copy: fetchSnippet.replacingOccurrences(of: "<token>", with: token))
                    Text("Same job ID twice prints once. GET /printers lists queues, GET /jobs/{id} returns the outcome.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Card(title: String(localized: "Check from Terminal"), systemImage: "terminal") {
                    let cmd = "curl -H 'Authorization: Bearer \(shownToken)' \(model.endpoint)/printers"
                    snippet(cmd, copy: cmd.replacingOccurrences(of: "<token>", with: token))
                }
            }
            .padding(24)
            .frame(maxWidth: 780, alignment: .leading)
        }
        .confirmationDialog("Issue a new access token?", isPresented: $confirmRotate) {
            Button("New token", role: .destructive) { model.rotateToken() }
        } message: {
            Text("Every app using the current token is disconnected until you update it.")
        }
    }

    private func snippet(_ text: String, copy: String) -> some View {
        HStack(alignment: .top) {
            Text(verbatim: text).font(.caption.monospaced()).textSelection(.enabled)
                .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.black.opacity(0.85), in: RoundedRectangle(cornerRadius: 8))
                .foregroundStyle(.green)
            Button { model.copy(copy, notice: String(localized: "Copied")) } label: { Image(systemName: "doc.on.doc") }
                .buttonStyle(.borderless)
        }
    }
}

// MARK: - File inspector

struct InspectorView: View {
    @State private var file: URL?
    @State private var preview: LabelPreview?
    @State private var targeted = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                dropZone
                if let file {
                    Card(title: file.lastPathComponent, systemImage: "doc") {
                        LabelPreviewView(preview: preview, maxHeight: 380)
                        Text(verbatim: file.path).font(.caption.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
                    }
                }
            }
            .padding(24)
        }
    }

    private var dropZone: some View {
        VStack(spacing: 10) {
            Image(systemName: "square.and.arrow.down.on.square")
                .font(.system(size: 34)).foregroundStyle(targeted ? Color.accentColor : .secondary)
            Text("Drop a label file here").font(.headline)
            Text("A .prn, .zpl or .ezpl file, or bytes saved from a print job. EZPL (Q) and ZPL (^GFA) bitmaps are redrawn; text commands are shown as text.")
                .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center).frame(maxWidth: 440)
            Button("Choose File…") { choose() }.buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity)
        .padding(28)
        .background(targeted ? Color.accentColor.opacity(0.08) : Color.clear, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(style: StrokeStyle(lineWidth: 1.5, dash: [6, 5]))
            .foregroundStyle(targeted ? Color.accentColor : Color.secondary.opacity(0.5)))
        .onDrop(of: [.fileURL], isTargeted: $targeted) { providers in
            _ = providers.first?.loadObject(ofClass: URL.self) { url, _ in
                if let url { Task { @MainActor in load(url) } }
            }
            return true
        }
    }

    private func choose() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if panel.runModal() == .OK, let url = panel.url { load(url) }
    }

    private func load(_ url: URL) {
        file = url
        preview = Core.filePreview(path: url.path)
    }
}
