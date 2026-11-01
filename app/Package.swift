// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Dotline",
    defaultLocalization: "en",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(name: "CDotline", path: "Sources/CDotline"),
        .executableTarget(
            name: "Dotline",
            dependencies: ["CDotline"],
            exclude: ["Resources"],
            linkerSettings: [.unsafeFlags(["-L../core/target/release"])]
        ),
    ]
)
