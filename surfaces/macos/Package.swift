// swift-tools-version: 6.4
import PackageDescription

let package = Package(
  name: "NerveRibbonUI",
  platforms: [.macOS(.v14)],
  products: [
    .library(name: "NerveRibbonUI", targets: ["NerveRibbonUI"]),
    .library(name: "NerveMac", targets: ["NerveMac"]),
  ],
  targets: [
    .target(name: "NerveRibbonUI"),
    // The macOS app target compiles this tree too. The symlink keeps one copy.
    .target(
      name: "NerveMac",
      dependencies: ["NerveRibbonUI"],
      path: "Sources/NerveMac",
      exclude: [
        "App/NerveApp.swift",
        "Info.plist",
        "Nerve.entitlements",
        "Resources",
        ".DS_Store",
      ]
    ),
    .testTarget(name: "NerveMacTests", dependencies: ["NerveMac"]),
  ]
)
