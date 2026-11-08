import AppKit

let size: CGFloat = 1024
let image = NSImage(size: NSSize(width: size, height: size))
image.lockFocus()

let bounds = NSRect(x: 0, y: 0, width: size, height: size)
let tile = NSBezierPath(roundedRect: bounds.insetBy(dx: 56, dy: 56), xRadius: 232, yRadius: 232)

let gradient = NSGradient(colors: [
    NSColor(calibratedRed: 0.38, green: 0.61, blue: 1.00, alpha: 1),
    NSColor(calibratedRed: 0.10, green: 0.30, blue: 0.82, alpha: 1)
])!
gradient.draw(in: tile, angle: -75)

NSColor.white.withAlphaComponent(0.12).setFill()
NSBezierPath(ovalIn: NSRect(x: -180, y: 560, width: 1100, height: 700)).fill()

let config = NSImage.SymbolConfiguration(pointSize: 430, weight: .semibold)
    .applying(NSImage.SymbolConfiguration(paletteColors: [.white]))
if let symbol = NSImage(systemSymbolName: "headphones", accessibilityDescription: nil)?
    .withSymbolConfiguration(config) {
    let s = symbol.size
    symbol.draw(in: NSRect(x: (size - s.width) / 2, y: (size - s.height) / 2,
                           width: s.width, height: s.height))
}

image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let png = rep.representation(using: .png, properties: [:]) else {
    fatalError("couldn't render icon")
}
try png.write(to: URL(fileURLWithPath: "Resources/AppIcon-1024.png"))
print("wrote icon")
