import XCTest
@testable import SlightGateway

/// Coverage for the normalized rich ACP content and tool-update payloads added
/// alongside image/audio/resource content and tool diffs, terminals, locations,
/// and raw I/O.
final class RichContentTests: XCTestCase {
    private let codec = GatewayCodec()

    func testMessagePayloadDecodesRichContentBlocks() throws {
        let json = """
        {
          "role": "assistant",
          "text": "",
          "blocks": [
            {
              "id": "b1",
              "kind": "image",
              "text": "",
              "mime_type": "image/png",
              "data_base64": "aGVsbG8=",
              "uri": "file:///tmp/x.png"
            },
            {
              "id": "b2",
              "kind": "audio",
              "text": "",
              "mime_type": "audio/mpeg",
              "data_base64": "YXVkaW8="
            },
            {
              "id": "b3",
              "kind": "resource_link",
              "text": "",
              "uri": "file:///work/src/main.rs",
              "name": "src/main.rs",
              "title": "main.rs",
              "mime_type": "text/x-rust",
              "size": 120
            },
            {
              "id": "b4",
              "kind": "resource_text",
              "text": "fn main() {}",
              "uri": "file:///work/src/main.rs"
            },
            {
              "id": "b5",
              "kind": "resource_blob",
              "text": "",
              "uri": "file:///work/blob.bin",
              "data_base64": "AAAA"
            }
          ]
        }
        """
        let payload = try codec.decodeJSON(SessionMessagePayload.self, from: Data(json.utf8))
        XCTAssertEqual(payload.blocks.count, 5)

        XCTAssertEqual(payload.blocks[0].kind, .image)
        XCTAssertEqual(payload.blocks[0].mimeType, "image/png")
        XCTAssertEqual(payload.blocks[0].dataBase64, "aGVsbG8=")
        XCTAssertEqual(payload.blocks[0].uri, "file:///tmp/x.png")

        XCTAssertEqual(payload.blocks[1].kind, .audio)
        XCTAssertEqual(payload.blocks[1].mimeType, "audio/mpeg")

        XCTAssertEqual(payload.blocks[2].kind, .resourceLink)
        XCTAssertEqual(payload.blocks[2].name, "src/main.rs")
        XCTAssertEqual(payload.blocks[2].title, "main.rs")
        XCTAssertEqual(payload.blocks[2].size, 120)

        XCTAssertEqual(payload.blocks[3].kind, .resourceText)
        XCTAssertEqual(payload.blocks[3].text, "fn main() {}")

        XCTAssertEqual(payload.blocks[4].kind, .resourceBlob)
        XCTAssertEqual(payload.blocks[4].dataBase64, "AAAA")
    }

    func testUnknownContentKindDecodesAsText() throws {
        let json = """
        { "id": "b1", "kind": "hologram", "text": "future" }
        """
        let block = try codec.decodeJSON(ContentBlock.self, from: Data(json.utf8))
        XCTAssertEqual(block.kind, .text)
        XCTAssertEqual(block.text, "future")
    }

    func testToolCallPayloadDecodesContentLocationsAndRawIO() throws {
        let json = """
        {
          "id": "tool-1",
          "title": "edit_file",
          "kind": "edit",
          "status": "in_progress",
          "detail": "/tmp/work/src/main.rs:12",
          "content": [
            {
              "type": "content",
              "id": "tool-text",
              "kind": "text",
              "text": "patching main.rs"
            },
            {
              "type": "diff",
              "path": "/tmp/work/src/main.rs",
              "old_text": "fn main() {}",
              "new_text": "fn main() { run() }"
            },
            {
              "type": "terminal",
              "terminal_id": "term-1"
            }
          ],
          "locations": [
            { "path": "/tmp/work/src/main.rs", "line": 12 }
          ],
          "raw_input": { "path": "/tmp/work/src/main.rs" },
          "raw_output": { "exit": 0 }
        }
        """
        let payload = try codec.decodeJSON(ToolCallPayload.self, from: Data(json.utf8))
        XCTAssertEqual(payload.status, .inProgress)
        XCTAssertEqual(payload.content.count, 3)

        guard case .content(let block) = payload.content[0] else {
            return XCTFail("Expected content block")
        }
        XCTAssertEqual(block.kind, .text)
        XCTAssertEqual(block.text, "patching main.rs")

        guard case .diff(let diff) = payload.content[1] else {
            return XCTFail("Expected diff")
        }
        XCTAssertEqual(diff.path, "/tmp/work/src/main.rs")
        XCTAssertEqual(diff.oldText, "fn main() {}")
        XCTAssertEqual(diff.newText, "fn main() { run() }")

        guard case .terminal(let terminal) = payload.content[2] else {
            return XCTFail("Expected terminal")
        }
        XCTAssertEqual(terminal.terminalId, "term-1")

        XCTAssertEqual(payload.locations.first?.line, 12)
        XCTAssertEqual(payload.rawInput?["path"]?.stringValue, "/tmp/work/src/main.rs")
        XCTAssertEqual(payload.rawOutput?["exit"]?.intValue, 0)

        let toolCall = payload.toolCall()
        XCTAssertEqual(toolCall.content.count, 3)
        XCTAssertEqual(toolCall.locations.count, 1)
    }

    func testRichToolCallPayloadRoundTripsThroughCodec() throws {
        let payload = ToolCallPayload(
            id: "tool-1",
            title: "edit_file",
            kind: "edit",
            status: .inProgress,
            content: [
                .content(ContentBlock(id: "c1", kind: .text, text: "patching")),
                .diff(Diff(path: "/tmp/work/src/main.rs", oldText: "old", newText: "new")),
                .terminal(TerminalRef(terminalId: "term-1")),
            ],
            locations: [ToolLocation(path: "/tmp/work/src/main.rs", line: 12)]
        )
        let value = try codec.encodeToJSONValue(payload)
        let decoded = try codec.decodePayload(ToolCallPayload.self, from: value)
        XCTAssertEqual(decoded, payload)
        XCTAssertEqual(decoded?.content.count, 3)
    }

    func testToolCallMergeRetainsRichFieldsFromPartialUpdate() {
        let rich = ToolCall(
            id: "tool-1",
            title: "edit_file",
            kind: "edit",
            status: .inProgress,
            content: [.terminal(TerminalRef(terminalId: "term-1"))],
            locations: [ToolLocation(path: "/tmp/work/src/main.rs", line: 12)],
            rawInput: .object(["path": .string("/tmp/work/src/main.rs")]),
            rawOutput: .object(["exit": .number(0)])
        )
        let partial = ToolCall(id: "tool-1", title: "edit_file", status: .completed)

        let merged = rich.merging(partial)
        XCTAssertEqual(merged.status, .completed)
        XCTAssertEqual(merged.content, rich.content)
        XCTAssertEqual(merged.locations, rich.locations)
        XCTAssertEqual(merged.rawInput, rich.rawInput)
        XCTAssertEqual(merged.rawOutput, rich.rawOutput)
    }

    func testToolCallMergeAllowsExplicitRawOutputReplacement() {
        let initial = ToolCall(id: "tool-1", title: "run", rawOutput: .string("old"))
        let update = ToolCall(id: "tool-1", title: "run", rawOutput: .string("new"))
        XCTAssertEqual(initial.merging(update).rawOutput, .string("new"))
    }

    func testToolCallMergeRetainsTitleWhenPartialUpdateOmitsIt() {
        let initial = ToolCall(
            id: "tool-1",
            title: "Check Helsinki weather",
            kind: "weather",
            detail: "Helsinki"
        )
        let update = ToolCall(id: "tool-1", title: "", status: .completed)

        let merged = initial.merging(update)

        XCTAssertEqual(merged.title, "Check Helsinki weather")
        XCTAssertEqual(merged.kind, "weather")
        XCTAssertEqual(merged.detail, "Helsinki")
    }
}
