import SwiftUI

/// Native Markdown rendering for the transcript's common AI-output shapes.
/// Inline Markdown is parsed by Foundation; this view adds block layout for
/// headings, lists, quotes, and fenced code without a web view or dependency.
struct MarkdownBlockView: View {
    let text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(lines) { line in
                switch line.kind {
                case .blank:
                    Color.clear.frame(height: 4)

                case .paragraph(let text):
                    markdownText(text)

                case .heading(let level, let text):
                    markdownText(text)
                        .font(headingFont(for: level))
                        .bold()

                case .listItem(let marker, let text):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(marker)
                            .foregroundStyle(.secondary)
                        markdownText(text)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                case .quote(let text):
                    HStack(alignment: .top, spacing: 8) {
                        RoundedRectangle(cornerRadius: 1)
                            .fill(.tertiary)
                            .frame(width: 3)
                        markdownText(text)
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                case .code(let language, let code):
                    VStack(alignment: .leading, spacing: 6) {
                        if let language, !language.isEmpty {
                            Text(language)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Text(code)
                            .font(.system(.callout, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(10)
                    .background(Color.primary.opacity(0.06))
                    .clipShape(.rect(cornerRadius: 8))
                }
            }
        }
    }

    private var lines: [Line] {
        let sourceLines = text.split(separator: "\n", omittingEmptySubsequences: false).map(
            String.init)
        var result: [Line] = []
        var codeLanguage: String?
        var codeLines: [String] = []

        for sourceLine in sourceLines {
            let trimmed = sourceLine.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                if codeLanguage != nil {
                    result.append(
                        Line(
                            id: result.count,
                            kind: .code(codeLanguage, codeLines.joined(separator: "\n"))))
                    codeLanguage = nil
                    codeLines = []
                } else {
                    codeLanguage = String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                }
                continue
            }

            if codeLanguage != nil {
                codeLines.append(sourceLine)
                continue
            }

            result.append(Line(id: result.count, kind: kind(for: sourceLine)))
        }

        if codeLanguage != nil {
            result.append(
                Line(id: result.count, kind: .code(codeLanguage, codeLines.joined(separator: "\n")))
            )
        }
        return result
    }

    private func kind(for line: String) -> Line.Kind {
        if line.isEmpty { return .blank }

        let headingPrefix = line.prefix { $0 == "#" }
        if !headingPrefix.isEmpty, line.dropFirst(headingPrefix.count).first == " " {
            return .heading(
                min(headingPrefix.count, 4),
                String(line.dropFirst(headingPrefix.count + 1))
            )
        }

        for marker in ["- ", "* ", "+ "] where line.hasPrefix(marker) {
            return .listItem("•", String(line.dropFirst(marker.count)))
        }

        if let ordered = orderedListItem(in: line) {
            return .listItem(ordered.marker, ordered.text)
        }

        if line.hasPrefix("> ") {
            return .quote(String(line.dropFirst(2)))
        }

        return .paragraph(line)
    }

    private func orderedListItem(in line: String) -> (marker: String, text: String)? {
        guard let separator = line.firstIndex(of: ".") else { return nil }
        let number = line[..<separator]
        let remainder = line[line.index(after: separator)...]
        guard !number.isEmpty,
            number.allSatisfy(\.isNumber),
            remainder.first == " "
        else { return nil }
        return (String(number) + ".", String(remainder.dropFirst()))
    }

    private func markdownText(_ source: String) -> Text {
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        if let markdown = try? AttributedString(markdown: source, options: options) {
            return Text(markdown)
        }
        return Text(source)
    }

    private func headingFont(for level: Int) -> Font {
        switch level {
        case 1: .title2
        case 2: .title3
        case 3: .headline
        default: .subheadline
        }
    }
}

extension MarkdownBlockView {
    fileprivate struct Line: Identifiable {
        enum Kind {
            case blank
            case paragraph(String)
            case heading(Int, String)
            case listItem(String, String)
            case quote(String)
            case code(String?, String)
        }

        let id: Int
        let kind: Kind
    }
}
