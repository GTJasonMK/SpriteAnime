use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

#[test]
fn multipart_reference_names_preserve_order_and_reject_unknown_mime() {
    assert_eq!(
        multipart_reference_file_name(0, "image/png").unwrap(),
        "reference_01.png"
    );
    assert_eq!(
        multipart_reference_file_name(1, "image/jpeg").unwrap(),
        "reference_02.jpg"
    );
    assert!(multipart_reference_file_name(0, "image/webp")
        .unwrap_err()
        .contains("MIME 类型不受支持"));
}

#[tokio::test]
async fn multipart_image_edit_sends_two_ordered_image_fields() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let receiver = std::thread::spawn(move || capture_request_body(listener));
    let api_base = format!("http://{address}/v1");
    let request = ImageApiRequest {
        api_base: &api_base,
        api_key: "key",
        prompt: "continue",
        model: "image-model",
        count: 1,
        size: "1024x1024",
        aspect_ratio: "1:1",
        resolution: "1K",
        proxy_url: "",
    };

    let result = call_images_edits_multipart_api(
        &request,
        &[(b"first", "image/png"), (b"second", "image/jpeg")],
    )
    .await
    .unwrap();
    let body = receiver.join().unwrap();

    assert_eq!(result, vec!["YWJj"]);
    assert_eq!(body.matches("name=\"image\"").count(), 2);
    let first = body.find("filename=\"reference_01.png\"").unwrap();
    let second = body.find("filename=\"reference_02.jpg\"").unwrap();
    assert!(first < second);
    assert!(body.contains("first"));
    assert!(body.contains("second"));
}

fn capture_request_body(listener: TcpListener) -> String {
    let (mut stream, _) = listener.accept().unwrap();
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        request.extend_from_slice(&buffer[..count]);
        if let Some(index) = request.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .or_else(|| line.strip_prefix("Content-Length: "))
        })
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap();
    while request.len() - header_end < content_length {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        request.extend_from_slice(&buffer[..count]);
    }
    let body = String::from_utf8_lossy(&request[header_end..]).to_string();
    let response_body = r#"{"data":[{"b64_json":"YWJj"}]}"#;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    )
    .unwrap();
    body
}
