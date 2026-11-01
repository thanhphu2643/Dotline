import CDotline
import Foundation

struct Printer: Decodable, Identifiable, Hashable {
    let name: String
    let state: String
    let isDefault: Bool
    var id: String { name }
}

struct Job: Decodable, Identifiable {
    let id: String
    let printer: String
    let bytes: Int
    let state: String
    let detail: String?
    let requestId: String?
    let createdAt: TimeInterval
    var payloadHash: String?

    var date: Date { Date(timeIntervalSince1970: createdAt) }
    var failed: Bool { state == "failed" }
    var isTest: Bool { id.hasPrefix("test-") }
}

struct LabelMedia: Decodable, Hashable {
    var widthMm: Double?
    var heightMm: Double?
}

/// Exactly what was sent, read back by the Rust decoder.
struct LabelPreview: Decodable {
    let kind: String  // bitmap | text | none
    var language: String?
    var widthDots: Int?
    var heightDots: Int?
    var widthBytes: Int?
    var data: String?
    var text: String?
    var media: LabelMedia?
    var error: String?
}

struct Snapshot: Decodable {
    let running: Bool
    var port: Int?
    var token: String?
    var printers: [Printer]?
    var printersError: String?
    var jobs: [Job]?
}

struct Outcome: Decodable {
    let ok: Bool?
    let error: String?
    let token: String?
    let state: String?
    let detail: String?
}

/// Thin Swift face over the Rust C ABI; every call returns owned JSON.
enum Core {
    private static func take<T: Decodable>(_ ptr: UnsafeMutablePointer<CChar>?, as type: T.Type) -> T? {
        guard let ptr else { return nil }
        defer { dotline_string_free(ptr) }
        return try? JSONDecoder().decode(T.self, from: Data(String(cString: ptr).utf8))
    }

    static func start(dataDir: URL, port: UInt16 = 0) -> Outcome? {
        take(dotline_start(dataDir.path, port), as: Outcome.self)
    }
    static func stop() { dotline_stop() }
    static func snapshot() -> Snapshot? { take(dotline_snapshot(), as: Snapshot.self) }
    static func rotateToken() -> Outcome? { take(dotline_rotate_token(), as: Outcome.self) }
    static func testPrint(printer: String, language: String) -> Outcome? {
        take(dotline_test_print(printer, language), as: Outcome.self)
    }
    static func clearJobs() { dotline_clear_jobs() }
    static func jobPreview(id: String) -> LabelPreview? { take(dotline_job_preview(id), as: LabelPreview.self) }
    static func filePreview(path: String) -> LabelPreview? { take(dotline_file_preview(path), as: LabelPreview.self) }
}
