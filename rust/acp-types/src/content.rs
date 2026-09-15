//! Normalized, agent-neutral rich content and tool-update summaries.
//!
//! ACP content blocks are MCP-compatible tagged payloads: text, image, audio,
//! resource links, and embedded resources. Tool calls additionally carry
//! content (including file diffs and terminal references), file locations, and
//! raw input/output. Native clients must never interpret those tag shapes
//! directly, so this module flattens the wire forms into small types that
//! `session-core` maps into the gateway contract.
//!
//! Raw `_meta`, annotations, and unknown variants are dropped here. Binary
//! payloads (image, audio, blob resources) are kept as base64 strings so a
//! client can render them, but they are always labelled with a MIME type so the
//! client can decide whether it understands the format. Nothing in this module
//! performs I/O or resolves a URI.

use crate::wire::{
    BlobResourceContents, ContentBlock, Diff, EmbeddedResource, EmbeddedResourceResource,
    ResourceLink, Terminal, TextResourceContents, ToolCallContent, ToolCallLocation,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A normalized content block carried by a message or tool-call update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NormalizedContent {
    /// Plain or Markdown text.
    Text { text: String },
    /// Base64-encoded image data with its MIME type.
    Image {
        data_base64: String,
        mime_type: String,
        uri: Option<String>,
    },
    /// Base64-encoded audio data with its MIME type.
    Audio {
        data_base64: String,
        mime_type: String,
    },
    /// A reference to a resource the agent can read, without its contents.
    ResourceLink(ResourceLinkSummary),
    /// Resource contents embedded directly in the message.
    Resource(ResourceContent),
}

impl NormalizedContent {
    /// Converts one conformant ACP content block, dropping unknown variants.
    pub fn from_wire(block: &ContentBlock) -> Option<Self> {
        match block {
            ContentBlock::Text(text) => Some(Self::Text {
                text: text.text.clone(),
            }),
            ContentBlock::Image(image) => Some(Self::Image {
                data_base64: image.data.clone(),
                mime_type: image.mime_type.clone(),
                uri: image.uri.clone(),
            }),
            ContentBlock::Audio(audio) => Some(Self::Audio {
                data_base64: audio.data.clone(),
                mime_type: audio.mime_type.clone(),
            }),
            ContentBlock::ResourceLink(link) => {
                Some(Self::ResourceLink(ResourceLinkSummary::from(link)))
            }
            ContentBlock::Resource(resource) => {
                Some(Self::Resource(ResourceContent::from(resource)))
            }
            _ => None,
        }
    }

    /// A plain-text representation for clients that only understand text.
    ///
    /// Media blocks collapse to empty strings (the client renders the block by
    /// kind); resource links fall back to their display name and URI.
    pub fn text_fallback(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Image { .. } | Self::Audio { .. } => String::new(),
            Self::ResourceLink(link) => match &link.title {
                Some(title) => format!("{title} ({})", link.uri),
                None => format!("{} ({})", link.name, link.uri),
            },
            Self::Resource(resource) => match resource {
                ResourceContent::Text { text, .. } => text.clone(),
                ResourceContent::Blob { uri, .. } => uri.clone(),
            },
        }
    }
}

/// A resource reference the agent can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourceLinkSummary {
    pub uri: String,
    pub name: String,
    pub mime_type: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub size: Option<i64>,
}

impl From<&ResourceLink> for ResourceLinkSummary {
    fn from(link: &ResourceLink) -> Self {
        Self {
            uri: link.uri.clone(),
            name: link.name.clone(),
            mime_type: link.mime_type.clone(),
            title: link.title.clone(),
            description: link.description.clone(),
            size: link.size,
        }
    }
}

/// Embedded resource contents, either text or base64-encoded binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resource_kind", rename_all = "snake_case")]
pub enum ResourceContent {
    Text {
        uri: String,
        text: String,
        mime_type: Option<String>,
    },
    Blob {
        uri: String,
        data_base64: String,
        mime_type: Option<String>,
    },
}

impl From<&EmbeddedResource> for ResourceContent {
    fn from(resource: &EmbeddedResource) -> Self {
        match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text) => {
                ResourceContent::from_text(text)
            }
            EmbeddedResourceResource::BlobResourceContents(blob) => {
                ResourceContent::from_blob(blob)
            }
            _ => ResourceContent::Text {
                uri: String::new(),
                text: String::new(),
                mime_type: None,
            },
        }
    }
}

impl ResourceContent {
    fn from_text(text: &TextResourceContents) -> Self {
        Self::Text {
            uri: text.uri.clone(),
            text: text.text.clone(),
            mime_type: text.mime_type.clone(),
        }
    }

    fn from_blob(blob: &BlobResourceContents) -> Self {
        Self::Blob {
            uri: blob.uri.clone(),
            data_base64: blob.blob.clone(),
            mime_type: blob.mime_type.clone(),
        }
    }
}

/// Normalized content produced by a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NormalizedToolContent {
    /// A standard content block (text, image, audio, or resource).
    Content(Box<NormalizedContent>),
    /// A file modification shown as a diff.
    Diff(DiffSummary),
    /// A reference to a terminal that produced output.
    Terminal(TerminalRef),
}

impl NormalizedToolContent {
    /// Converts one conformant ACP tool-call content entry, dropping unknown
    /// variants.
    pub fn from_wire(content: &ToolCallContent) -> Option<Self> {
        match content {
            ToolCallContent::Content(block) => NormalizedContent::from_wire(&block.content)
                .map(|content| Self::Content(Box::new(content))),
            ToolCallContent::Diff(diff) => Some(Self::Diff(DiffSummary::from(diff))),
            ToolCallContent::Terminal(terminal) => {
                Some(Self::Terminal(TerminalRef::from(terminal)))
            }
            _ => None,
        }
    }
}

/// A normalized file diff from a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiffSummary {
    pub path: String,
    pub old_text: Option<String>,
    pub new_text: String,
}

impl From<&Diff> for DiffSummary {
    fn from(diff: &Diff) -> Self {
        Self {
            path: diff.path.display().to_string(),
            old_text: diff.old_text.clone(),
            new_text: diff.new_text.clone(),
        }
    }
}

/// A reference to a terminal that produced tool output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TerminalRef {
    pub terminal_id: String,
}

impl From<&Terminal> for TerminalRef {
    fn from(terminal: &Terminal) -> Self {
        Self {
            terminal_id: terminal.terminal_id.0.to_string(),
        }
    }
}

/// A file location touched by a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolLocation {
    pub path: String,
    pub line: Option<u32>,
}

impl From<&ToolCallLocation> for ToolLocation {
    fn from(location: &ToolCallLocation) -> Self {
        Self {
            path: location.path.display().to_string(),
            line: location.line,
        }
    }
}

/// Converts a raw tool-output value, keeping it only when it is present and not
/// `null`.
pub fn normalized_raw(value: Option<&Value>) -> Option<Value> {
    value.filter(|value| !value.is_null()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{
        AudioContent, ImageContent, TextContent, ToolCallContent as WireToolCallContent,
        ToolContent as Content,
    };
    use serde_json::json;

    #[test]
    fn text_block_converts_to_text() {
        let block = ContentBlock::Text(TextContent::new("hello"));
        assert_eq!(
            NormalizedContent::from_wire(&block),
            Some(NormalizedContent::Text {
                text: "hello".to_string()
            })
        );
    }

    #[test]
    fn image_block_keeps_base64_and_mime_type() {
        let image = ImageContent::new("aGVsbG8=", "image/png").uri("file:///tmp/x.png");
        let block = ContentBlock::Image(image);
        match NormalizedContent::from_wire(&block).unwrap() {
            NormalizedContent::Image {
                data_base64,
                mime_type,
                uri,
            } => {
                assert_eq!(data_base64, "aGVsbG8=");
                assert_eq!(mime_type, "image/png");
                assert_eq!(uri.as_deref(), Some("file:///tmp/x.png"));
            }
            other => panic!("expected image, got {other:?}"),
        }
    }

    #[test]
    fn audio_block_keeps_base64_and_mime_type() {
        let block = ContentBlock::Audio(AudioContent::new("YXVkaW8=", "audio/mpeg"));
        assert_eq!(
            NormalizedContent::from_wire(&block),
            Some(NormalizedContent::Audio {
                data_base64: "YXVkaW8=".to_string(),
                mime_type: "audio/mpeg".to_string(),
            })
        );
        assert_eq!(
            NormalizedContent::from_wire(&block)
                .unwrap()
                .text_fallback(),
            ""
        );
    }

    #[test]
    fn resource_link_summarizes_fields() {
        let link = ResourceLink::new("src/main.rs", "file:///work/src/main.rs")
            .title("main.rs")
            .mime_type("text/x-rust")
            .size(120);
        let block = ContentBlock::ResourceLink(link);
        match NormalizedContent::from_wire(&block).unwrap() {
            NormalizedContent::ResourceLink(summary) => {
                assert_eq!(summary.name, "src/main.rs");
                assert_eq!(summary.uri, "file:///work/src/main.rs");
                assert_eq!(summary.title.as_deref(), Some("main.rs"));
                assert_eq!(summary.size, Some(120));
            }
            other => panic!("expected resource link, got {other:?}"),
        }
    }

    #[test]
    fn embedded_text_resource_converts() {
        let resource = EmbeddedResource::new(EmbeddedResourceResource::TextResourceContents(
            TextResourceContents::new("fn main() {}", "file:///work/src/main.rs"),
        ));
        let block = ContentBlock::Resource(resource);
        match NormalizedContent::from_wire(&block).unwrap() {
            NormalizedContent::Resource(ResourceContent::Text { text, uri, .. }) => {
                assert_eq!(text, "fn main() {}");
                assert_eq!(uri, "file:///work/src/main.rs");
            }
            other => panic!("expected text resource, got {other:?}"),
        }
    }

    #[test]
    fn embedded_blob_resource_converts() {
        let resource = EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
            BlobResourceContents::new("AAAA", "file:///work/blob.bin"),
        ));
        let block = ContentBlock::Resource(resource);
        match NormalizedContent::from_wire(&block).unwrap() {
            NormalizedContent::Resource(ResourceContent::Blob {
                data_base64, uri, ..
            }) => {
                assert_eq!(data_base64, "AAAA");
                assert_eq!(uri, "file:///work/blob.bin");
            }
            other => panic!("expected blob resource, got {other:?}"),
        }
    }

    #[test]
    fn tool_call_diff_and_terminal_convert() {
        let diff = WireToolCallContent::Diff(
            Diff::new("/work/src/main.rs", "fn main() {}").old_text("fn main() {}"),
        );
        let terminal = WireToolCallContent::Terminal(Terminal::new("term-1"));
        assert_eq!(
            NormalizedToolContent::from_wire(&diff),
            Some(NormalizedToolContent::Diff(DiffSummary {
                path: "/work/src/main.rs".to_string(),
                old_text: Some("fn main() {}".to_string()),
                new_text: "fn main() {}".to_string(),
            }))
        );
        assert_eq!(
            NormalizedToolContent::from_wire(&terminal),
            Some(NormalizedToolContent::Terminal(TerminalRef {
                terminal_id: "term-1".to_string(),
            }))
        );
    }

    #[test]
    fn tool_call_text_content_converts() {
        let content = WireToolCallContent::Content(Content::new("output"));
        assert_eq!(
            NormalizedToolContent::from_wire(&content),
            Some(NormalizedToolContent::Content(Box::new(
                NormalizedContent::Text {
                    text: "output".to_string()
                }
            )))
        );
    }

    #[test]
    fn location_includes_optional_line() {
        let location = ToolCallLocation::new("/work/src/main.rs").line(42);
        assert_eq!(
            ToolLocation::from(&location),
            ToolLocation {
                path: "/work/src/main.rs".to_string(),
                line: Some(42),
            }
        );
    }

    #[test]
    fn raw_null_is_dropped() {
        assert_eq!(normalized_raw(None), None);
        assert_eq!(normalized_raw(Some(&Value::Null)), None);
        assert_eq!(
            normalized_raw(Some(&json!({"exit": 0}))),
            Some(json!({"exit": 0}))
        );
    }
}
