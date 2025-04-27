use reqwest;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum NgrokError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error("Ngrok tunnel not found")]
    NgrokTunelNotFound,
}

#[derive(Deserialize)]
struct Tunnel {
    public_url: String,
    name: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    tunnels: Vec<Tunnel>,
}

pub async fn fetch_ngrok_url() -> Result<String, NgrokError> {
    let resp = reqwest::get("http://127.0.0.1:4040/api/tunnels")
        .await?
        .json::<ApiResponse>()
        .await?;

    Ok(resp
        .tunnels
        .into_iter()
        .find(|t| t.name == "holland2stay-bot")
        .map(|t| t.public_url)
        .ok_or_else(|| NgrokError::NgrokTunelNotFound)?)
}
