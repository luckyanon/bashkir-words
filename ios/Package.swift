// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BashkirWords",
    platforms: [.iOS(.v17)],
    products: [.library(name: "BashkirWords", targets: ["BashkirWords"])],
    targets: [.target(name: "BashkirWords")],
    swiftLanguageModes: [.v5]
)
