import SwiftUI

enum Section: String, CaseIterable, Identifiable {
    case overview, jobs, printers, pairing, inspector
    var id: String { rawValue }
    var title: String {
        switch self {
        case .overview: String(localized: "Overview")
        case .jobs: String(localized: "Jobs")
        case .printers: String(localized: "Printers")
        case .pairing: String(localized: "Connect")
        case .inspector: String(localized: "Inspect File")
        }
    }
    var icon: String {
        switch self {
        case .overview: "square.grid.2x2"
        case .jobs: "list.bullet.rectangle.portrait"
        case .printers: "printer"
        case .pairing: "link"
        case .inspector: "doc.viewfinder"
        }
    }
}

struct Card<Content: View>: View {
    var title: String?
    var systemImage: String?
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let title {
                Label(title, systemImage: systemImage ?? "circle")
                    .font(.headline)
                    .labelStyle(.titleAndIcon)
            }
            content
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.separator.opacity(0.6)))
    }
}

struct StatCard: View {
    let title: String
    let value: String
    let systemImage: String
    var tint: Color = .accentColor

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: systemImage)
                .font(.title2)
                .foregroundStyle(tint)
                .frame(width: 40, height: 40)
                .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(value).font(.title2.weight(.semibold)).monospacedDigit()
                Text(title).font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(.background, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.separator.opacity(0.6)))
    }
}

struct StateBadge: View {
    let text: String
    let color: Color
    var body: some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 7).padding(.vertical, 3)
            .foregroundStyle(color)
            .background(color.opacity(0.14), in: Capsule())
    }
}

extension Printer {
    var stateText: String {
        switch state {
        case "idle": String(localized: "Ready")
        case "printing": String(localized: "Printing")
        case "disabled": String(localized: "Paused")
        default: String(localized: "Unknown")
        }
    }
    var stateColor: Color {
        switch state {
        case "idle": .green
        case "printing": .blue
        case "disabled": .orange
        default: .secondary
        }
    }
}

extension Job {
    var stateText: String { failed ? String(localized: "Failed") : String(localized: "Sent") }
    var stateColor: Color { failed ? .red : .green }
}

func formatBytes(_ n: Int) -> String {
    let f = ByteCountFormatter()
    f.countStyle = .file
    f.allowsNonnumericFormatting = false  // "0 KB", not "Zero KB"
    return f.string(fromByteCount: Int64(n))
}

// MARK: - Label rendering

extension LabelPreview {
    /// 1-bit raster, 1 = black (printer polarity), drawn on white.
    var image: CGImage? {
        guard kind == "bitmap", let w = widthDots, let h = heightDots, let wb = widthBytes,
              let b64 = data, let bytes = Data(base64Encoded: b64), bytes.count >= wb * h,
              let provider = CGDataProvider(data: bytes as CFData) else { return nil }
        return CGImage(
            width: w, height: h, bitsPerComponent: 1, bitsPerPixel: 1, bytesPerRow: wb,
            space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0),
            provider: provider, decode: [1, 0], shouldInterpolate: false, intent: .defaultIntent)
    }

    var languageName: String {
        switch language {
        case "zpl": "ZPL · Zebra"
        case "ezpl": "EZPL · GoDEX"
        case "tspl": "TSPL · TSC"
        default: String(localized: "Raw")
        }
    }

    var sizeText: String? {
        var parts: [String] = []
        if let w = media?.widthMm, let h = media?.heightMm {
            parts.append("\(w.formatted()) × \(h.formatted()) mm")
        }
        if let w = widthDots, let h = heightDots { parts.append("\(w) × \(h) dots") }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }
}

/// The label as it leaves Dotline: white stock, crisp dots, a soft shadow.
struct LabelPreviewView: View {
    let preview: LabelPreview?
    var maxHeight: CGFloat = 320
    @State private var zoom: CGFloat = 1

    var body: some View {
        Group {
            if let preview, let image = preview.image {
                VStack(spacing: 10) {
                    ScrollView([.horizontal, .vertical]) {
                        Image(decorative: image, scale: 1)
                            .interpolation(.none)
                            .resizable()
                            .aspectRatio(contentMode: .fit)
                            .frame(maxWidth: 460 * zoom, maxHeight: maxHeight * zoom)
                            .padding(6)
                            .background(.white, in: RoundedRectangle(cornerRadius: 6))
                            .shadow(color: .black.opacity(0.18), radius: 8, y: 3)
                            .padding(20)
                            .frame(maxWidth: .infinity)
                    }
                    .frame(maxHeight: maxHeight + 60)
                    .background(checker)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    HStack {
                        StateBadge(text: preview.languageName, color: .purple)
                        if let size = preview.sizeText {
                            Text(size).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button { zoom = max(0.5, zoom - 0.25) } label: { Image(systemName: "minus.magnifyingglass") }
                        Text("\(Int(zoom * 100))%").font(.caption.monospacedDigit()).frame(width: 40)
                        Button { zoom = min(4, zoom + 0.25) } label: { Image(systemName: "plus.magnifyingglass") }
                    }
                    .buttonStyle(.borderless)
                }
            } else if let preview, preview.kind == "text", let text = preview.text {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        StateBadge(text: preview.languageName, color: .purple)
                        Text("Text commands, no bitmap to preview")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    ScrollView {
                        Text(text)
                            .font(.system(.caption, design: .monospaced))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(12)
                    }
                    .frame(maxHeight: maxHeight)
                    .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.separator))
                }
            } else {
                ContentUnavailableView(
                    "No Preview",
                    systemImage: "photo.badge.exclamationmark",
                    description: Text(preview?.error ?? String(localized: "This job's data could not be read.")))
                    .frame(maxHeight: maxHeight)
            }
        }
    }

    private var checker: some View {
        Canvas { ctx, size in
            let s: CGFloat = 10
            for y in stride(from: 0, to: size.height, by: s) {
                for x in stride(from: 0, to: size.width, by: s) where (Int(x / s) + Int(y / s)).isMultiple(of: 2) {
                    ctx.fill(Path(CGRect(x: x, y: y, width: s, height: s)), with: .color(.gray.opacity(0.08)))
                }
            }
        }
        .background(Color(nsColor: .underPageBackgroundColor))
    }
}
