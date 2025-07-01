use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use futures::future::{LocalBoxFuture, Ready, ready};
use log::{info, warn};
use secrecy::{ExposeSecret, Secret};
use actix_web::http::{header, Uri};
pub struct TokenAuthMiddleware;

impl<S> Transform<S, ServiceRequest> for TokenAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Transform = TokenAuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TokenAuthMiddlewareService { service }))
    }
}

pub struct TokenAuthMiddlewareService<S> {
    service: S,
}

impl<S> Service<ServiceRequest> for TokenAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 获取路径
        let path = req.path();
        info!("收到请求 path={}", path);
        // 只保护 Git 操作端点
        let requires_auth = path.ends_with(".git")
            || path.contains("/info/refs")
            || path.contains("/git-upload-pack");
        // || path.contains("/HEAD");
        info!("requires_auth的结果是{:?}", requires_auth);
        // 获取有效 Token
        let valid_token = match std::env::var("GIT_SERVER_TOKEN") {
            Ok(token) => Secret::new(token),
            Err(_) => {
                warn!("GIT_SERVER_TOKEN not set, using default token");
                Secret::new("default_token".to_string())
            }
        };

        if requires_auth {
            info!("Request requires authentication: {}", path);

            // 1. 尝试从 Bearer Token 获取
            if let Some(auth_header) = req.headers().get("Authorization") {
                info!("当前auth_header为:{:?}", auth_header);
                if let Ok(auth_str) = auth_header.to_str() {
                    info!("Authorization header: {}", auth_str);

                    // Bearer 认证
                    if auth_str.starts_with("Bearer ") {
                        info!("进入Bearer 认证");
                        let token = &auth_str[7..];
                        info!("Bearer token: {}", token);

                        if token == valid_token.expose_secret() {
                            info!("Bearer token validation successful");
                            let fut = self.service.call(req);
                            return Box::pin(async move {
                                let res = fut.await?;
                                Ok(res)
                            });
                        } else {
                            warn!("Bearer token mismatch");
                        }
                    }
                    // Basic 认证
                    else if auth_str.starts_with("Basic ") {
                        info!("进入Basic 认证");
                        info!("Basic auth detected");

                        if let Ok(decoded) = BASE64_STANDARD.decode(&auth_str[6..]) {
                            if let Ok(creds) = String::from_utf8(decoded) {
                                info!("Decoded credentials: {}", creds);

                                // 格式为 "username:password"
                                let parts: Vec<&str> = creds.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    let password = parts[1];
                                    info!("Password extracted: {}", password);

                                    if password == valid_token.expose_secret() {
                                        info!("Basic auth validation successful");
                                        let fut = self.service.call(req);
                                        return Box::pin(async move {
                                            let res = fut.await?;
                                            Ok(res)
                                        });
                                    } else {
                                        warn!("Basic auth password mismatch");
                                    }
                                } else {
                                    warn!("Invalid Basic auth format");
                                }
                            }
                        }
                    }
                }
            }

            // 认证失败
            warn!("Authentication failed for {}", path);
            return Box::pin(async {
                Ok(req.into_response(
                    HttpResponse::Unauthorized()
                        .insert_header(("WWW-Authenticate", "Basic realm=\"Git Repository\""))
                        .body("Authentication failed. Please check your token."),
                ))
            });
        }

        // 非 Git 操作端点，直接放行
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}
