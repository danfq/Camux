import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3 else {
    fputs("usage: render_macos_icon.swift <logo.png> <output.png>\n", stderr)
    exit(2)
}

let canvasSize = 512
let scale = CGFloat(canvasSize) / 1024
let colorSpace = CGColorSpace(name: CGColorSpace.displayP3)!
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: canvasSize,
    pixelsHigh: canvasSize,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
)!

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)

let context = NSGraphicsContext.current!.cgContext
context.clear(CGRect(x: 0, y: 0, width: canvasSize, height: canvasSize))
context.setAllowsAntialiasing(true)
context.setShouldAntialias(true)
context.interpolationQuality = .high

let iconRect = CGRect(x: 64, y: 64, width: 896, height: 896).applying(
    CGAffineTransform(scaleX: scale, y: scale)
)
let background = CGPath(
    roundedRect: iconRect,
    cornerWidth: 224 * scale,
    cornerHeight: 224 * scale,
    transform: nil
)
context.addPath(background)
context.clip()

let colors = [
    CGColor(colorSpace: colorSpace, components: [0.23839, 0.75464, 0.80588, 1.0])!,
    CGColor(colorSpace: colorSpace, components: [0.83537, 0.26293, 0.84945, 1.0])!,
] as CFArray
let gradient = CGGradient(colorsSpace: colorSpace, colors: colors, locations: [0.3, 1.0])!
context.drawLinearGradient(
    gradient,
    start: CGPoint(x: 512 * scale, y: 64 * scale),
    end: CGPoint(x: 512 * scale, y: 960 * scale),
    options: []
)

context.resetClip()

guard let logo = NSImage(contentsOfFile: CommandLine.arguments[1]) else {
    fputs("could not load logo\n", stderr)
    exit(1)
}

logo.draw(
    in: NSRect(x: 174, y: 174, width: 676, height: 676).applying(
        CGAffineTransform(scaleX: scale, y: scale)
    ),
    from: .zero,
    operation: .sourceOver,
    fraction: 1.0
)

NSGraphicsContext.restoreGraphicsState()

guard let png = bitmap.representation(using: NSBitmapImageRep.FileType.png, properties: [:]) else {
    fputs("could not encode icon\n", stderr)
    exit(1)
}

try png.write(
    to: URL(fileURLWithPath: CommandLine.arguments[2]),
    options: Data.WritingOptions.atomic
)
