use actix_web::dev::{Service, Transform, ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures::future::{ok, Ready};
use log::info;

pub struct SimpleLogger;

impl<S, B> Transform<S, ServiceRequest> for SimpleLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SimpleLoggerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SimpleLoggerMiddleware { service })
    }
}

pub struct SimpleLoggerMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SimpleLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = futures::future::LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let method = req.method().to_string();

        info!("请求: {} {}", method, path);

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            info!("响应: {}", res.status());
            Ok(res)
        })
    }
}
