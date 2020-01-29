pub use async_trait::async_trait;
pub use jsonwebtoken::Validation;

use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use http::HeaderValue;
use jsonwebtoken::{dangerous_unsafe_decode, decode};
use roa_core::{Context, Model, Next, Status, StatusCode};
use serde::{de::DeserializeOwned, Serialize};

const INVALID_HEADER_VALUE: &str = r#"Bearer realm="<jwt>", error="invalid_token""#;

#[async_trait]
pub trait JwtState<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
{
    async fn get_validation(&self) -> Validation;
    async fn get_secret(&self, claim: &C) -> Result<Vec<u8>, Status>;
    async fn set_claim(&mut self, claim: C);
}

fn unauthorized_error<M: Model>(ctx: &mut Context<M>, www_authentication: &'static str) -> Status {
    ctx.response.headers.insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static(www_authentication),
    );
    Status::new(StatusCode::UNAUTHORIZED, "".to_string(), false)
}

fn try_get_token<M: Model>(ctx: Context<M>) -> Result<String, Status> {
    match ctx.request.headers.get(AUTHORIZATION) {
        None => Err(unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE)),
        Some(value) => {
            let token = value
                .to_str()
                .map_err(|_| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?;
            match token.find("Bearer") {
                None => Err(unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE)),
                Some(n) => Ok(token[n + 6..].trim().to_string()),
            }
        }
    }
}

// TODO: test it
pub async fn jwt_verify<M, C>(mut ctx: Context<M>, next: Next) -> Result<(), Status>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
    M::State: JwtState<M, C>,
{
    let token = try_get_token(ctx.clone())?;
    let dangerous_claim: C = dangerous_unsafe_decode(&token)
        .map_err(|_err| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?
        .claims;
    let secret = ctx.get_secret(&dangerous_claim).await?;
    let validation = ctx.get_validation().await;
    let claim: C = decode(&token, &secret, &validation)
        .map_err(|_err| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?
        .claims;
    ctx.set_claim(claim).await;
    next().await
}
