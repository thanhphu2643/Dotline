// Renders the Dotline app icon: swift make-icon.swift <out.png> [size]
import AppKit

let out = CommandLine.arguments[1]
let px = CommandLine.arguments.count > 2 ? Int(CommandLine.arguments[2])! : 1024
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
let cg = NSGraphicsContext.current!.cgContext
// Work in a 1024 top-left canvas.
let k = CGFloat(px) / 1024
cg.scaleBy(x: k, y: k)
cg.translateBy(x: 0, y: 1024)
cg.scaleBy(x: 1, y: -1)

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(red: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: a)
}
func rounded(_ r: CGRect, _ radius: CGFloat) -> CGPath {
    CGPath(roundedRect: r, cornerWidth: radius, cornerHeight: radius, transform: nil)
}
func linear(_ colors: [CGColor], _ from: CGPoint, _ to: CGPoint) {
    let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors as CFArray, locations: nil)!
    cg.drawLinearGradient(g, start: from, end: to, options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
}

// Body: macOS grid squircle with drop shadow.
let body = CGRect(x: 100, y: 100, width: 824, height: 824)
cg.saveGState()
cg.setShadow(offset: CGSize(width: 0, height: 14), blur: 28, color: rgb(0x000000, 0.28))
cg.addPath(rounded(body, 186)); cg.setFillColor(rgb(0x3B5BFF)); cg.fillPath()
cg.restoreGState()
cg.saveGState()
cg.addPath(rounded(body, 186)); cg.clip()
linear([rgb(0x2E7BFF), rgb(0x4A4DF5), rgb(0x8A3CF0)], CGPoint(x: 100, y: 100), CGPoint(x: 924, y: 924))
// Soft top sheen.
linear([rgb(0xFFFFFF, 0.22), rgb(0xFFFFFF, 0)], CGPoint(x: 512, y: 100), CGPoint(x: 512, y: 520))

// Label stock, emerging from the printer slot.
let label = CGRect(x: 262, y: 300, width: 500, height: 470)
cg.saveGState()
cg.setShadow(offset: CGSize(width: 0, height: 18), blur: 34, color: rgb(0x10124A, 0.35))
cg.addPath(rounded(label, 44)); cg.setFillColor(rgb(0xFFFFFF)); cg.fillPath()
cg.restoreGState()
cg.saveGState()
cg.addPath(rounded(label, 44)); cg.clip()
linear([rgb(0xFFFFFF), rgb(0xEEF1FA)], CGPoint(x: 512, y: 300), CGPoint(x: 512, y: 770))
cg.restoreGState()

// The dot line: print dots, the last one still being printed.
let dots: [UInt32] = [0x2E7BFF, 0x3F66FA, 0x5A52F4, 0x7742F0, 0x8A3CF0]
for (i, color) in dots.enumerated() {
    let d: CGFloat = 62, gap: CGFloat = 26
    let x = 512 - (d * 5 + gap * 4) / 2 + CGFloat(i) * (d + gap)
    cg.addEllipse(in: CGRect(x: x, y: 430, width: d, height: d))
    cg.setFillColor(rgb(color, i == 4 ? 0.35 : 1))
    cg.fillPath()
}
// Text lines.
cg.setFillColor(rgb(0xC9D0E3))
cg.addPath(rounded(CGRect(x: 323, y: 552, width: 300, height: 30), 15)); cg.fillPath()
cg.setFillColor(rgb(0xDCE1EE))
cg.addPath(rounded(CGRect(x: 323, y: 604, width: 200, height: 30), 15)); cg.fillPath()
// Tear perforation.
cg.setFillColor(rgb(0xC9D0E3))
var x: CGFloat = 300
while x < 730 { cg.addEllipse(in: CGRect(x: x, y: 692, width: 12, height: 12)); x += 30 }
cg.fillPath()

// Printer slot over the label's top edge.
let slot = CGRect(x: 196, y: 262, width: 632, height: 76)
cg.saveGState()
cg.setShadow(offset: CGSize(width: 0, height: 10), blur: 18, color: rgb(0x0A0C3A, 0.45))
cg.addPath(rounded(slot, 38)); cg.setFillColor(rgb(0x171A4D)); cg.fillPath()
cg.restoreGState()
cg.addPath(rounded(slot.insetBy(dx: 30, dy: 26), 12)); cg.setFillColor(rgb(0x05061F)); cg.fillPath()
// Status LED.
cg.addEllipse(in: CGRect(x: 760, y: 288, width: 22, height: 22)); cg.setFillColor(rgb(0x3CF08A)); cg.fillPath()
cg.restoreGState()

NSGraphicsContext.current = nil
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
