use super::{sanitize_markdown_data_uri_images, MultimodalContentPart};

#[test]
fn sanitizer_replaces_markdown_data_uri_with_placeholder() {
    let encoded = "AQIDBA==";
    let sanitized = sanitize_markdown_data_uri_images(&format!(
        "before ![fixture.png](data:image/png;base64,{encoded}) after"
    ))
    .expect("image should be detected");

    assert_eq!(
        sanitized.text,
        "before [attached image: fixture.png, image/png] after"
    );
    assert_eq!(sanitized.images.len(), 1);
    assert_eq!(sanitized.images[0].alt_text, "fixture.png");
    assert_eq!(sanitized.images[0].mime_type, "image/png");
    assert!(sanitized.images[0].estimated_tokens >= 85);
    assert!(!sanitized.text.contains(encoded));
}

#[test]
fn split_keeps_image_parts_for_provider_serializers() {
    let parts = super::split_markdown_data_uri_images(
        "![fixture.png](data:image/png;base64,AQIDBA==)\n\nRead it.",
    )
    .expect("image should be detected");

    assert!(matches!(
        parts.as_slice(),
        [
            MultimodalContentPart::Image {
                alt_text: "fixture.png",
                mime_type: "image/png",
                data: "AQIDBA==",
            },
            MultimodalContentPart::Text("Read it.")
        ]
    ));
}
