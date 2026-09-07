#[derive(serde::Deserialize)]
pub(crate) struct CaptureTarget {
    pub(crate) name: Option<String>,
    pub(crate) path: Option<String>,
}

pub(crate) async fn request_capture(
    client: &reqwest::Client,
    x: i32,
    y: i32,
) -> Result<CaptureTarget, String> {
    let response = client.get("http://127.0.0.1:19841/capture")
        .query(&[("x", x), ("y", y)])
        .send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("{status}: {body}"));
    }
    serde_json::from_str::<CaptureTarget>(&body).map_err(|error| error.to_string())
}
