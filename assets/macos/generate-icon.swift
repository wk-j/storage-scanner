// Original icon source. Regenerate with scripts/package-macos.sh --icon-only.
import AppKit
let directory = CommandLine.arguments[1]
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels,
            pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
            isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
        let p = CGFloat(pixels)
        NSColor(calibratedRed: 0.055, green: 0.065, blue: 0.09, alpha: 1).setFill()
        NSBezierPath(roundedRect: NSRect(x: p * 0.04, y: p * 0.04,
            width: p * 0.92, height: p * 0.92), xRadius: p * 0.2, yRadius: p * 0.2).fill()
        let colors: [NSColor] = [.systemBlue, .systemPurple, .systemPink, .systemOrange, .darkGray]
        for column in 0..<10 {
            for row in 0..<4 {
                colors[column / 2].setFill()
                NSBezierPath(roundedRect: NSRect(x: p * (0.14 + Double(column) * 0.073),
                    y: p * (0.31 + Double(row) * 0.10), width: p * 0.05, height: p * 0.07),
                    xRadius: p * 0.012, yRadius: p * 0.012).fill()
            }
        }
        NSGraphicsContext.restoreGraphicsState()
        let suffix = scale == 2 ? "@2x" : ""
        let url = URL(fileURLWithPath: directory).appendingPathComponent("icon_\(size)x\(size)\(suffix).png")
        try bitmap.representation(using: .png, properties: [:])!.write(to: url)
    }
}
