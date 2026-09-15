import SlightGateway
import SwiftUI

#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

public struct MessageRowView: View {
    public let message: SessionMessage
    public let isThinkingActive: Bool

    public init(message: SessionMessage, isThinkingActive: Bool = false) {
        self.message = message
        self.isThinkingActive = isThinkingActive
    }

    public var body: some View {
        switch message.role {
        case .assistant:
            assistantMessage
        case .user:
            userMessage
        case .system:
            systemMessage
        }
    }

    private var assistantMessage: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !thinkingBlocks.isEmpty || isThinkingActive {
                ThinkingDisclosure(
                    blocks: thinkingBlocks,
                    isActive: isThinkingActive,
                    completedDuration: message.reasoningDuration
                )
            }
            ForEach(contentBlocks) { block in
                blockView(block)
            }
            if message.isStreaming {
                ProgressView()
                    .controlSize(.mini)
                    .padding(.top, 2)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var userMessage: some View {
        HStack {
            Spacer(minLength: 36)
            VStack(alignment: .trailing, spacing: 4) {
                ForEach(message.blocks) { block in
                    blockView(block)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.primary.opacity(message.id.hasPrefix("local-") ? 0.04 : 0.08))
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .opacity(message.id.hasPrefix("local-") ? 0.55 : 1)
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
    }

    private var systemMessage: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(message.blocks) { block in
                blockView(block)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.secondary.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var thinkingBlocks: [ContentBlock] {
        message.blocks.filter { $0.kind == .reasoning }
    }

    private var contentBlocks: [ContentBlock] {
        message.blocks.filter { $0.kind != .reasoning }
    }

    @ViewBuilder
    private func blockView(_ block: ContentBlock) -> some View {
        switch block.kind {
        case .code:
            Text(block.text)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        case .reasoning:
            Text(block.text)
                .font(.callout)
                .italic()
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
        case .text, .toolResult, .resourceText:
            MarkdownBlockView(text: block.text)
                .font(.body)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        case .image:
            MediaBlockView(block: block, systemImage: "photo")
        case .audio:
            MediaBlockView(block: block, systemImage: "waveform")
        case .resourceLink:
            ResourceLabelView(block: block, systemImage: "link")
        case .resourceBlob:
            ResourceLabelView(block: block, systemImage: "doc")
        }
    }

}

private struct ThinkingDisclosure: View {
    let blocks: [ContentBlock]
    let isActive: Bool
    let completedDuration: TimeInterval?
    @State private var startedAt = Date.now
    @State private var isPresented = false

    var body: some View {
        Button {
            isPresented = true
        } label: {
            HStack(spacing: 6) {
                statusLabel
                Image(systemName: "chevron.right")
                    .font(.caption2)
            }
            .foregroundStyle(.secondary)
        }
        .buttonStyle(.plain)
        .popover(isPresented: $isPresented) {
            ScrollView {
                MarkdownBlockView(text: blocks.map(\.text).joined())
                    .textSelection(.enabled)
                    .padding(16)
                    .frame(minWidth: 280, maxWidth: 520, alignment: .leading)
            }
            .frame(maxHeight: 360)
            .presentationCompactAdaptation(.popover)
        }
        .onAppear { startedAt = .now }
    }

    @ViewBuilder
    private var statusLabel: some View {
        if isActive {
            statusLabel(isActive: true)
        } else {
            statusLabel(isActive: false)
        }
    }

    private func statusLabel(isActive: Bool) -> some View {
        HStack(spacing: 6) {
            if isActive {
                ProgressView().controlSize(.mini)
            } else {
                Image(systemName: "checkmark")
            }
            Text(label(isActive: isActive))
                .font(.caption)
        }
    }

    private func label(isActive: Bool) -> String {
        if let summary {
            if isActive { return "Thinking: \(summary)" }
            return "Thought: \(summary) (\(format(completedDuration ?? duration)))"
        }
        return isActive ? "thinking" : "thought (\(format(completedDuration ?? duration)))"
    }

    private var summary: String? {
        let text = blocks
            .map(\.text)
            .joined(separator: " ")
            .split(whereSeparator: { $0.isWhitespace || $0.isNewline })
            .joined(separator: " ")
        guard !text.isEmpty else { return nil }
        if text.count <= 48 { return text }
        return String(text.prefix(45)) + "..."
    }

    private func format(_ duration: TimeInterval) -> String {
        if duration < 60 {
            return duration.formatted(.number.precision(.fractionLength(1))) + "s"
        }
        let seconds = Int(duration)
        return "\(seconds / 60)m \(seconds % 60)s"
    }

    private var duration: TimeInterval {
        max(0, Date.now.timeIntervalSince(startedAt))
    }
}

/// Renders an image or audio block. Images are decoded from base64 locally;
/// audio is labelled only, since this client does not play agent media.
struct MediaBlockView: View {
    let block: ContentBlock
    let systemImage: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if block.kind == .image, let image = decodedImage {
                image
                    .resizable()
                    .scaledToFit()
                    .frame(maxHeight: 240)
                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
            } else {
                HStack(spacing: 6) {
                    Image(systemName: systemImage)
                    Text(caption)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }
            }
        }
        .padding(6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.05))
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private var caption: String {
        let kind = block.kind == .audio ? "Audio" : "Image"
        let mime = block.mimeType ?? "unknown type"
        if let uri = block.uri {
            return "\(kind) (\(mime)) — \(uri)"
        }
        return "\(kind) (\(mime))"
    }

    private var decodedImage: Image? {
        guard let encoded = block.dataBase64, let data = Data(base64Encoded: encoded) else {
            return nil
        }
        #if canImport(UIKit)
        guard let uiImage = UIImage(data: data) else { return nil }
        return Image(uiImage: uiImage)
        #elseif canImport(AppKit)
        guard let nsImage = NSImage(data: data) else { return nil }
        return Image(nsImage: nsImage)
        #else
        return nil
        #endif
    }
}

/// Renders a resource link or embedded resource as labelled text. The URI is
/// shown but never opened or fetched implicitly.
struct ResourceLabelView: View {
    let block: ContentBlock
    let systemImage: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                Image(systemName: systemImage)
                Text(block.title ?? block.name ?? block.uri ?? "Resource")
                    .font(.callout.weight(.medium))
                    .textSelection(.enabled)
            }
            if let uri = block.uri {
                Text(uri)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }
            if let description = block.description, !description.isEmpty {
                Text(description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.05))
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }
}
