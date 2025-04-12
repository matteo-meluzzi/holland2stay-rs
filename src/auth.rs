use std::{collections::HashMap, sync::Arc};

use reqwest::{Client, Url, cookie, cookie::CookieStore};

use crate::api::Holland2StayError;

#[derive(derive_new::new)]
pub struct Auth {
    username: String,
    password: String,
}

#[derive(derive_new::new)]
pub struct Login {
    client: Client,
    bearer_token: String,
}

fn holland2stay_base_url() -> Url {
    Url::parse("https://holland2stay.com").expect("could not parse holland2stay.com")
}

pub fn build_client() -> Client {
    let cookie_store = Arc::new(cookie::Jar::default());
    Client::builder()
        .cookie_provider(cookie_store.clone())
        .build()
        .expect("Could not build http client")
}

async fn initiate_session(client: &Client) -> Result<(), reqwest::Error> {
    let url = holland2stay_base_url()
        .join("/api/auth/session")
        .expect("Could not parse session url");
    let _ = client.get(url).send().await?.error_for_status()?;
    Ok(())
}

async fn get_csfr_token(client: &Client) -> Result<String, Holland2StayError> {
    let url = holland2stay_base_url()
        .join("api/auth/csrf")
        .expect("could not parse csfr url");
    let response = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    fn parse_response(response: &serde_json::Value) -> Option<String> {
        Some(
            response
                .as_object()?
                .get("csrfToken")?
                .as_str()?
                .to_string(),
        )
    }
    parse_response(&response)
        .ok_or_else(|| Holland2StayError::ConversionError("Could not parse csfr token".to_string()))
}

async fn login(client: &Client, auth: &Auth, token: &str) -> Result<String, Holland2StayError> {
    let url = holland2stay_base_url()
        .join("api/auth/callback/credentials")
        .expect("could not parse login url");
    let form_body = HashMap::from([
        ("username", auth.username.as_str()),
        ("password", auth.password.as_str()),
        ("csrfToken", token),
    ]);

    let _ = client
        .post(url)
        .form(&form_body)
        .send()
        .await?
        .error_for_status()?;

    let url = holland2stay_base_url()
        .join("api/auth/session")
        .expect("could not parse session url");
    let response = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    fn parse_bearer_token(response: &serde_json::Value) -> Option<String> {
        Some(
            response
                .as_object()?
                .get("accessToken")?
                .as_str()?
                .to_string(),
        )
    }
    parse_bearer_token(&response).ok_or_else(|| {
        Holland2StayError::ConversionError(
            "Could not parse json session response into bearer token".to_string(),
        )
    })
}

pub async fn login_holland2stay(auth: &Auth) -> Result<Login, Holland2StayError> {
    let client = build_client();
    initiate_session(&client).await?;
    let csfr_token = get_csfr_token(&client).await?;
    let bearer_token = login(&client, auth, &csfr_token).await?;
    Ok(Login::new(client, bearer_token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initiate_session() {
        let client = build_client();
        initiate_session(&client).await.unwrap();
        // if let Some(cookie) = session.cookie_store.cookies(&holland2stay_base_url()) {
        //     println!("cookie: {:?}", cookie);
        // }
    }

    #[tokio::test]
    async fn test_get_csfr_token() {
        let client = build_client();
        initiate_session(&client).await.unwrap();
        let token = get_csfr_token(&client).await.unwrap();
        println!("token: {token}")
    }

    #[tokio::test]
    async fn test_login() {
        let client = build_client();
        initiate_session(&client).await.unwrap();
        let csfr_token = get_csfr_token(&client).await.unwrap();
        let bearer_token = login(
            &client,
            &Auth::new(
                "matteo@meluzzi.com".to_string(),
                r#"4Td(\@)]vSFot^15]\jC/ir(i,iW<}H6fpLx9i`wPF"#.to_string(),
            ),
            &csfr_token,
        )
        .await
        .unwrap();
        println!("bearer token: {bearer_token}")
    }
}
