#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultimodalContentPart<'a> {
    Text(&'a str),
    Image {
        alt_text: &'a str,
        mime_type: &'a str,
        data: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedImageSummary {
    pub alt_text: String,
    pub mime_type: String,
    pub estimated_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedMarkdownContent {
    pub text: String,
    pub images: Vec<EmbeddedImageSummary>,
}

pub fn split_markdown_data_uri_images(content: &str) -> Option<Vec<MultimodalContentPart<'_>>> {
    let mut parts = Vec::new();
    let mut search_from = 0;
    let mut text_start = 0;
    let mut found_image = false;

    while let Some(relative_start) = content[search_from..].find("![") {
        let image_start = search_from + relative_start;
        let alt_start = image_start + 2;
        let Some(close_alt_relative) = content[alt_start..].find("](") else {
            search_from = alt_start;
            continue;
        };
        let url_start = alt_start + close_alt_relative + 2;
        if !content[url_start..].starts_with("data:") {
            search_from = url_start;
            continue;
        }

        let Some(close_url_relative) = content[url_start..].find(')') else {
            break;
        };
        let url_end = url_start + close_url_relative;
        let url = &content[url_start..url_end];

        let Some((mime_type, data)) = parse_image_data_uri(url) else {
            search_from = url_end + 1;
            continue;
        };
        let alt_text = content[alt_start..alt_start + close_alt_relative].trim();

        push_text_part(&mut parts, &content[text_start..image_start]);
        parts.push(MultimodalContentPart::Image {
            alt_text,
            mime_type,
            data,
        });
        found_image = true;
        text_start = url_end + 1;
        search_from = text_start;
    }

    if !found_image {
        return None;
    }

    push_text_part(&mut parts, &content[text_start..]);
    Some(parts)
}

pub fn sanitize_markdown_data_uri_images(content: &str) -> Option<SanitizedMarkdownContent> {
    let parts = split_markdown_data_uri_images(content)?;
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    for part in parts {
        match part {
            MultimodalContentPart::Text(text) => text_parts.push(text.to_string()),
            MultimodalContentPart::Image {
                alt_text,
                mime_type,
                data,
            } => {
                images.push(EmbeddedImageSummary {
                    alt_text: alt_text.to_string(),
                    mime_type: mime_type.to_string(),
                    estimated_tokens: estimate_data_uri_image_tokens(data),
                });
                text_parts.push(image_placeholder(alt_text, mime_type));
            }
        }
    }

    Some(SanitizedMarkdownContent {
        text: text_parts.join(" "),
        images,
    })
}

pub fn estimate_data_uri_image_tokens(base64_data: &str) -> u64 {
    const BASE_IMAGE_TOKENS: u64 = 85;
    const ESTIMATED_BYTES_PER_TOKEN: u64 = 768;

    let trimmed = base64_data.trim();
    let encoded_len = trimmed.chars().filter(|ch| !ch.is_whitespace()).count();
    if encoded_len == 0 {
        return BASE_IMAGE_TOKENS;
    }

    let padding = trimmed.chars().rev().take_while(|ch| *ch == '=').count();
    let decoded_bytes = ((encoded_len / 4) * 3).saturating_sub(padding) as u64;
    BASE_IMAGE_TOKENS + decoded_bytes.div_ceil(ESTIMATED_BYTES_PER_TOKEN)
}

fn parse_image_data_uri(uri: &str) -> Option<(&str, &str)> {
    let payload = uri.strip_prefix("data:")?;
    let (mime_type, data) = payload.split_once(";base64,")?;
    if mime_type.starts_with("image/") && !data.trim().is_empty() {
        Some((mime_type, data))
    } else {
        None
    }
}

fn push_text_part<'a>(parts: &mut Vec<MultimodalContentPart<'a>>, text: &'a str) {
    let text = text.trim();
    if !text.is_empty() {
        parts.push(MultimodalContentPart::Text(text));
    }
}

fn image_placeholder(alt_text: &str, mime_type: &str) -> String {
    if alt_text.is_empty() {
        format!("[attached image: {mime_type}]")
    } else {
        format!("[attached image: {alt_text}, {mime_type}]")
    }
}

#[cfg(test)]
#[path = "content_parts_tests.rs"]
mod tests;
