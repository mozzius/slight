// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SlightKit",
    defaultLocalization: "en",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(name: "SlightGateway", targets: ["SlightGateway"]),
        .library(name: "SlightGatewayUI", targets: ["SlightGatewayUI"]),
    ],
    targets: [
        .target(
            name: "SlightGateway",
            path: "Sources/SlightGateway",
            exclude: ["Protocol/PROVISIONAL.md"]
        ),
        .target(
            name: "SlightGatewayUI",
            dependencies: ["SlightGateway"],
            path: "Sources/SlightGatewayUI"
        ),
        .testTarget(
            name: "SlightGatewayTests",
            dependencies: ["SlightGateway", "SlightGatewayUI"],
            path: "Tests/SlightGatewayTests"
        ),
    ]
)
