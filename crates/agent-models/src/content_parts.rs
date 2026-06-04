#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MultimodalContentPart<'a> {
    Text(&'a str),
    Image { mime_type: &'a str, data: &'a str },
}

pub(crate) fn split_markdown_data_uri_images(
    content: &str,
) -> Option<Vec<MultimodalContentPart<'_>>> {
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

        push_text_part(&mut parts, &content[text_start..image_start]);
        parts.push(MultimodalContentPart::Image { mime_type, data });
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
