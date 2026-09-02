use std::collections::BTreeMap;

pub const STATE_URL: &str = "http://127.0.0.1:19841/state";

pub async fn request_state(
    client: &reqwest::Client,
    keys: &[String],
) -> Result<BTreeMap<String, String>, String> {
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
    serde_json::from_str::<BTreeMap<String, String>>(
        &response.text().await.map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}
