// swift-tools-version: 6.4
import PackageDescription

let package = Package(
  name: "NerveRibbonUI",
  platforms: [.macOS(.v14)],
  products: [
    .library(name: "NerveRibbonUI", targets: ["NerveRibbonUI"]),
  ],
  targets: [
    .target(name: "NerveRibbonUI"),
  ]
)
