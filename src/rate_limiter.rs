use axum::{
    body::{box_body, Body, BoxBody},
    http::{Request, Response},
};
use futures::future::BoxFuture;
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use std::sync::Arc;
use tower::Service;

#[derive(Clone)]
pub struct RateLimiterMiddleware<S> {
    rate_limiter: Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>,
    inner: S,
    quota: Quota,
}

impl<S> RateLimiterMiddleware<S> {
    pub fn new(inner: S, quota: Quota) -> Self {
        RateLimiterMiddleware {
            rate_limiter: Arc::new(RateLimiter::dashmap(quota)),
            inner,
            quota,
        }
    }
}

impl<S, ReqBody> Service<Request<ReqBody>> for RateLimiterMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = Response<BoxBody>;

    type Error = S::Error;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // https://github.com/tower-rs/tower/issues/547
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        let rate_limiter = self.rate_limiter.clone();
        let quota = self.quota;

        Box::pin(async move {
            let ip = req
                .headers()
                .get("x-forwarded-for")
                .map(|d| d.to_str().ok())
                .flatten()
                .unwrap_or("unknown")
                .to_owned();

            if rate_limiter.check_key(&ip).is_err() {
                let res = Response::builder()
                    .status(429)
                    .body(box_body(Body::from(format!(
                        "Rate limit of API calls exceeded. {:?}",
                        quota
                    ))))
                    .expect("Couldn't build body.");

                tracing::warn!(%ip, "Rate limited.");

                return Ok(res);
            }

            let res = inner.call(req).await?;

            Ok(res)
        })
    }
}
