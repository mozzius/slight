import SlightGateway
import SwiftUI

/// A cell-based transcript that retains visible-cell identity while reading
/// history and gives the tail scroller sole ownership during live streaming.
struct SessionTranscriptView: View {
    let items: [TranscriptItem]
    let isAgentWorking: Bool
    let isLoadingHistory: Bool
    let hasOlderHistory: Bool
    let hasLoadedHistory: Bool
    let isReplaying: Bool
    let showsEmptyState: Bool
    let onLoadOlder: () async -> Void

    @State private var scrollTargetID: String?
    @State private var scrollMetrics = TranscriptScrollMetrics.defaultValue
    @State private var isFollowingTail = true
    @State private var hasPositionedInitially = false
    @State private var historyRequestInFlight = false

    private static let coordinateSpaceName = "session-transcript"
    private static let bottomID = "session-transcript-bottom"
    private static let paginationDistance = 120.0
    private static let tailDistance = 80.0

    var body: some View {
        GeometryReader { viewport in
            ScrollViewReader { scrollProxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        ProgressView("Loading earlier messages")
                            .controlSize(.small)
                            .labelStyle(.iconOnly)
                            .opacity(hasOlderHistory && isLoadingHistory ? 1 : 0)
                            .accessibilityHidden(!hasOlderHistory || !isLoadingHistory)
                            .frame(maxWidth: .infinity)
                            .frame(height: 20)
                            .background {
                                GeometryReader { geometry in
                                    Color.clear.preference(
                                        key: TranscriptScrollMetrics.self,
                                        value: .top(
                                            geometry.frame(in: .named(Self.coordinateSpaceName))
                                                .minY
                                        )
                                    )
                                }
                            }

                        if items.isEmpty, !isAgentWorking, showsEmptyState {
                            ContentUnavailableView(
                                "No messages yet",
                                systemImage: "bubble.left.and.bubble.right",
                                description: Text("Send the first prompt to start the agent.")
                            )
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.top, 40)
                        }

                        ForEach(items) { item in
                            TranscriptCellView(
                                item: item,
                                isThinkingActive: isThinkingActive(for: item)
                            )
                            .equatable()
                            .id(item.id)
                        }

                        Color.clear
                            .frame(height: 12)
                            .id(Self.bottomID)
                            .background {
                                GeometryReader { geometry in
                                    Color.clear.preference(
                                        key: TranscriptScrollMetrics.self,
                                        value: .bottom(
                                            geometry.frame(in: .named(Self.coordinateSpaceName))
                                                .maxY
                                        )
                                    )
                                }
                            }
                    }
                    .scrollTargetLayout()
                    .padding(.horizontal, 16)
                    .padding(.top, 4)
                    .frame(maxWidth: 800)
                    .frame(maxWidth: .infinity, minHeight: viewport.size.height, alignment: .top)
                }
                #if os(iOS)
                    .scrollDismissesKeyboard(.interactively)
                #endif
                .coordinateSpace(name: Self.coordinateSpaceName)
                .modifier(BottomScrollEdgeEffect())
                .scrollPosition(id: $scrollTargetID)
                .onPreferenceChange(TranscriptScrollMetrics.self) { metrics in
                    scrollMetrics = metrics
                    if metrics.hasBottomMeasurement {
                        isFollowingTail = metrics.bottom <= viewport.size.height + Self.tailDistance
                    }
                    loadOlderHistoryIfNeeded()
                }
                .task(id: hasLoadedHistory) {
                    guard hasLoadedHistory, !hasPositionedInitially else { return }
                    await Task.yield()
                    scrollToTail(using: scrollProxy)
                    await Task.yield()
                    hasPositionedInitially = true
                }
                .task(id: items.first?.id) {
                    loadOlderHistoryIfNeeded()
                }
                .onChange(of: items) { _, _ in
                    guard hasPositionedInitially,
                        isFollowingTail,
                        !isLoadingHistory,
                        !isReplaying
                    else { return }
                    scrollToTail(using: scrollProxy)
                }
                .onChange(of: isReplaying) { wasReplaying, isReplaying in
                    guard wasReplaying,
                        !isReplaying,
                        hasPositionedInitially,
                        isFollowingTail
                    else { return }
                    scrollToTail(using: scrollProxy)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func isThinkingActive(for item: TranscriptItem) -> Bool {
        guard isAgentWorking,
            items.last?.id == item.id,
            case .message(let message) = item
        else { return false }
        return message.role == .assistant && message.blocks.last?.kind == .reasoning
    }

    private func loadOlderHistoryIfNeeded() {
        guard hasPositionedInitially,
            scrollMetrics.top >= -Self.paginationDistance,
            scrollMetrics.top <= Self.paginationDistance,
            hasOlderHistory,
            !isLoadingHistory,
            !historyRequestInFlight
        else { return }

        historyRequestInFlight = true
        Task { @MainActor in
            await onLoadOlder()
            historyRequestInFlight = false
        }
    }

    private func scrollToTail(using scrollProxy: ScrollViewProxy) {
        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction) {
            scrollProxy.scrollTo(Self.bottomID, anchor: .bottom)
        }
    }
}
