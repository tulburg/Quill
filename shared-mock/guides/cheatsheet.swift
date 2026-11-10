// Quill: draws assets/quill-cheatsheet.pdf (A4, one page).
// Run: swift shared-mock/guides/cheatsheet.swift shared-mock/guides/assets/quill-cheatsheet.pdf
import AppKit

let out = URL(fileURLWithPath: CommandLine.arguments[1])
var box = CGRect(x: 0, y: 0, width: 595, height: 842)
let ctx = CGContext(out as CFURL, mediaBox: &box, [kCGPDFContextCreator: "Quill"] as CFDictionary)!
ctx.beginPDFPage(nil)
NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)

let ink = NSColor(srgbRed: 0.13, green: 0.13, blue: 0.15, alpha: 1)
let soft = NSColor(srgbRed: 0.45, green: 0.45, blue: 0.48, alpha: 1)
let accent = NSColor(srgbRed: 0.36, green: 0.45, blue: 0.72, alpha: 1)
func draw(_ s: String, _ x: CGFloat, _ top: CGFloat, _ font: NSFont, _ color: NSColor, kern: CGFloat = 0) {
    let a = NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: color, .kern: kern])
    a.draw(at: NSPoint(x: x, y: 842 - top - font.ascender))
}

let left: CGFloat = 47, right: CGFloat = 268
draw("Quill", left, 40, .boldSystemFont(ofSize: 28), ink)
draw("A quick reference for writing on your Mac", left, 80, .systemFont(ofSize: 12), ink)

let sections: [(String, [(String, String)])] = [
    ("GET AROUND", [("Command + N", "New document"), ("Command + Shift + N", "New note window"), ("Command + J", "Search your library"),
                    ("Command + ]", "Show or hide sidebar"), ("Command + ,", "Open settings"), ("Command + /", "View all shortcuts")]),
    ("WRITE AND EDIT", [("Command + B / I / U", "Bold / Italic / Underline"), ("Command + K", "Add or edit a link"), ("Command + F", "Find in the document"),
                        ("Command + Shift + H", "Find and replace"), ("Command + Z", "Undo"), ("Command + Shift + Z", "Redo"),
                        ("Command + Enter", "Toggle a task"), ("Tab / Shift + Tab", "Indent / Outdent")]),
    ("INSERT BLOCKS", [("/", "Open the insert menu"), ("Command + Option + 1-9", "Heading level 1-9"), ("Command + Option + 0", "Body text"),
                       ("Command + Option + A", "Insert a table"), ("Command + Option + C", "Insert a code block")]),
]
var y: CGFloat = 125
for (title, rows) in sections {
    draw(title, left, y, .boldSystemFont(ofSize: 10), accent, kern: 0.3)
    y += 23
    for (keys, what) in rows {
        draw(keys, left, y, .systemFont(ofSize: 10), ink)
        draw(what, right, y, .systemFont(ofSize: 10), ink)
        y += 19.5
    }
    y += 18
}
draw("Default shortcuts. Customize them in Settings > Shortcuts.", left, 754, .systemFont(ofSize: 8.5), soft)
draw("Your library: ~/Documents/Quill", left, 770, .systemFont(ofSize: 8.5), soft)

NSGraphicsContext.current = nil
ctx.endPDFPage()
ctx.closePDF()
