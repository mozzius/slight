import SlightGateway
import SwiftUI

/// One stable lazy-stack cell. Streaming updates replace only the affected
/// message or tool cell instead of invalidating the entire transcript layout.
struct TranscriptCellView: View, Equatable {
    let item: TranscriptItem
    let isThinkingActive: Bool

    var body: some View {
        switch item {
        case .message(let message):
            MessageRowView(message: message, isThinkingActive: isThinkingActive)
        case .toolCall(let toolCall):
            ToolCallRowView(toolCall: toolCall)
        case .notice(let notice):
            NoticeRowView(notice: notice)
        }
    }
}
