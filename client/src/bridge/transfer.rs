pub(crate) async fn transfer(
    client: &reqwest::Client,
    source: String,
    destination: String,
) -> Result<(), String> {
    let response = client
        .post("http://127.0.0.1:19841/transfer")
        .query(&[("source", source), ("destination", destination)])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("transfer failed ({status}): {body}"));
    }
    Ok(())
}
