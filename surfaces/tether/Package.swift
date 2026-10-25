// swift-tools-version: 6.2
import PackageDescription

// Hosts the macOS app's panel. The web directory is the page a Tether that
// does not link this product can still install.
let package = Package(
  name: "NervePlugin",
  platforms: [.macOS(.v26), .iOS(.v26)],
  products: [
    .library(name: "NervePlugin", targets: ["NervePlugin"]),
  ],
  dependencies: [
    .package(path: "../../../Tether/app/Packages/TetherFrontend"),
    .package(path: "../macos"),
  ],
  targets: [
    .target(
      name: "NervePlugin",
      dependencies: [
        .product(name: "TetherPluginKit", package: "TetherFrontend"),
        .product(name: "NerveMac", package: "macos", condition: .when(platforms: [.macOS])),
      ]
    ),
  ]
)
