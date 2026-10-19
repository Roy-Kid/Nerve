// swift-tools-version: 6.2
import PackageDescription

// Development smoke test against the sibling Tether checkout. The installed
// plugin itself is only ../../surfaces/tether, with no Swift or native dependencies.
let package = Package(name: "NerveTetherTests", platforms: [.macOS(.v26)],
  dependencies: [.package(path: "../../../Tether/app/Packages/TetherPluginHost"),
    .package(path: "../../../Tether/app/Packages/TetherFrontend")],
  targets: [.testTarget(name: "NerveTetherTests", dependencies: [
    .product(name: "TetherPluginHost", package: "TetherPluginHost"),
    .product(name: "TetherPluginKit", package: "TetherFrontend")
  ])])
