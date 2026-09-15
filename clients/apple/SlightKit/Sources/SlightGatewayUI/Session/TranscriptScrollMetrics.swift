import SwiftUI

struct TranscriptScrollMetrics: Equatable, PreferenceKey {
    var top = CGFloat.greatestFiniteMagnitude
    var bottom = -CGFloat.greatestFiniteMagnitude

    static let defaultValue = TranscriptScrollMetrics()

    var hasBottomMeasurement: Bool {
        bottom > -CGFloat.greatestFiniteMagnitude
    }

    static func top(_ value: CGFloat) -> TranscriptScrollMetrics {
        TranscriptScrollMetrics(top: value)
    }

    static func bottom(_ value: CGFloat) -> TranscriptScrollMetrics {
        TranscriptScrollMetrics(bottom: value)
    }

    static func reduce(
        value: inout TranscriptScrollMetrics,
        nextValue: () -> TranscriptScrollMetrics
    ) {
        let next = nextValue()
        value.top = min(value.top, next.top)
        value.bottom = max(value.bottom, next.bottom)
    }
}
