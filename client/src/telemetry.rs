pub const STATE_URL: &str = "http://127.0.0.1:19841/state";

#[cfg(test)]
pub(crate) static TEST_SERVER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub async fn request_state(
    client: &reqwest::Client,
    keys: &[String],
) -> Result<String, String> {
    let mut request = client.get(STATE_URL);
    if !keys.is_empty() {
        request = request.query(
            &keys.iter().map(|key| ("key", key)).collect::<Vec<_>>(),
        );
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    let response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let body = response.text().await.map_err(|error| error.to_string())?;
    let value = serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("state response must be a JSON object".to_owned());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, thread};

    fn serve_once(status: &str, body: &str) -> thread::JoinHandle<String> {
        let listener = TcpListener::bind("127.0.0.1:19841").unwrap();
        let body = body.to_owned();
        let status = status.to_owned();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let length = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..length]).to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            request
        })
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_state_preserves_mixed_json_and_selected_query() {
        let _guard = TEST_SERVER_LOCK.lock().unwrap();
        let raw = r#"{"score":"1e3","enabled":true,"nested":{"value":null},"items":[1,"two",false]}"#;
        let server = serve_once("200 OK", raw);
        let result = request_state(&client(), &["score".to_owned(), "eternity.dtpSpent".to_owned()]).await;
        let request = server.join().unwrap();

        assert_eq!(result.unwrap(), raw);
        assert_eq!(request.lines().next().unwrap(), "GET /state?key=score&key=eternity.dtpSpent HTTP/1.1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_state_rejects_http_malformed_and_non_object_responses() {
        let _guard = TEST_SERVER_LOCK.lock().unwrap();
        let server = serve_once("500 Internal Server Error", r#"{"error":"failed"}"#);
        assert!(request_state(&client(), &[]).await.is_err());
        server.join().unwrap();

        for body in ["not json", "[]", "null"] {
            let server = serve_once("200 OK", body);
            assert!(request_state(&client(), &[]).await.is_err());
            server.join().unwrap();
        }
    }
}
