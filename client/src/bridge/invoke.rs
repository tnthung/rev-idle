pub(crate) async fn invoke(
    client: &reqwest::Client,
    name: String,
) -> Result<(), String> {
    let response = client
        .post("http://127.0.0.1:19841/invoke")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("invoke failed ({status}): {body}"));
    }
    Ok(())
}
