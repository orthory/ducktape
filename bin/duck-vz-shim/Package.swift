// swift-tools-version:5.9
// The macOS VMM shim: Firecracker's seat on a Mac. See Sources/main.swift.
import PackageDescription

let package = Package(
    name: "duck-vz-shim",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "duck-vz-shim",
            path: "Sources"
        )
    ]
)
