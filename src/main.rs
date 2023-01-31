#![deny(warnings)]
mod ctl;
mod store;
mod util;

use util::http as httputil;

use bytes::{Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header;
use hyper::server::conn::http1;
use hyper::{Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use store::{Car, CarStore, MemCarStore, SQLiteCarStore, StoreError};
use tokio::net::TcpListener;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

const INTERNAL_SERVER_ERROR: &str = "Internal Server Error";

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn mk_err_response<T: Into<Bytes>>(code: StatusCode, body: T) -> Response<BoxBody> {
    Response::builder().status(code).body(full(body)).unwrap()
}

fn mk_json_response<T>(value: &T) -> Response<BoxBody>
where
    T: ?Sized + Serialize,
{
    match serde_json::to_string(value) {
        Ok(json) => Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(full(json))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(full(INTERNAL_SERVER_ERROR))
            .unwrap(),
    }
}

async fn decode_request_body<T: DeserializeOwned>(req: Request<Incoming>) -> Result<T, String> {
    match req.collect().await {
        Ok(bytes) => {
            let buf = bytes.aggregate();
            let mut de = serde_json::Deserializer::from_reader(buf.reader());
            match T::deserialize(&mut de) {
                Ok(body) => Ok(body),
                Err(e) => Err(format!("failed to parse request body: {}", e)),
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/*
`impl<T> From<std::result::Result<T,String>> for Response<BoxBody>` then we can do this in Svc
    fn list_images(r: Request<Incoming>) -> Response<BoxBody> {
        ctl::list_images()?
    }
but compiler complains:
  only traits defined in the current crate can be implemented for types defined outside of the crate
so it turns to
    fn list_images(r: Request<Incoming>) -> Response<BoxBody> {
        ret_to_resp(ctl::list_images())
    }
*/
fn ret_to_resp<T: serde::Serialize>(v: std::result::Result<T, String>) -> Response<BoxBody> {
    match v {
        Ok(t) => mk_json_response(&t),
        Err(e) => mk_err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Clone)]
struct Svc {
    mux: std::sync::Arc<Router>,
    car_store: std::sync::Arc<dyn CarStore + Send + Sync>,
}

impl Svc {
    fn store_err_to_resp(err: StoreError) -> Response<BoxBody> {
        match err {
            StoreError::NotFound(err_msg) => mk_err_response(StatusCode::NOT_FOUND, err_msg),
            StoreError::Internal(err_msg) => {
                error!("{}", err_msg);
                mk_err_response(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_SERVER_ERROR)
            }
        }
    }

    async fn get_car_list(self, _: httputil::Context, _: Request<Incoming>) -> Response<BoxBody> {
        match self.car_store.get_all_cars() {
            Ok(cars) => mk_json_response(&cars),
            Err(e) => Svc::store_err_to_resp(e),
        }
    }

    async fn get_car_by_id(
        self,
        ctx: httputil::Context,
        _: Request<Incoming>,
    ) -> Response<BoxBody> {
        match ctx.vars.get("id") {
            Some(car_id) => {
                let id: u32 = match car_id.trim().parse() {
                    Ok(num) => num,
                    Err(_) => {
                        return mk_err_response(
                            StatusCode::BAD_REQUEST,
                            format!("invalid id={}, expect uint32 number", car_id),
                        )
                    }
                };
                match self.car_store.get_car(id) {
                    Ok(car) => mk_json_response(&car),
                    Err(store_err) => Self::store_err_to_resp(store_err),
                }
            }
            None => {
                return mk_err_response(StatusCode::BAD_REQUEST, format!("expect id in url path"))
            }
        }
    }

    async fn create_car(self, _: httputil::Context, req: Request<Incoming>) -> Response<BoxBody> {
        match decode_request_body::<Car>(req).await {
            Ok(new_car) => {
                if new_car.year <= 0 {
                    return mk_err_response(
                        StatusCode::BAD_REQUEST,
                        "car year must be greater than 0",
                    );
                }
                match self
                    .car_store
                    .create_car(new_car.brand, new_car.model, new_car.year)
                {
                    Ok(nid) => mk_json_response(&json!({ "id": nid }).to_string()),
                    Err(e) => Svc::store_err_to_resp(e),
                }
            }
            Err(e) => mk_err_response(StatusCode::BAD_REQUEST, format!("invalid json input:{e}")),
        }
    }

    async fn update_car(self, ctx: httputil::Context, req: Request<Incoming>) -> Response<BoxBody> {
        let car_id = match ctx.vars.get("id") {
            Some(car_id) => match car_id.trim().parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    return mk_err_response(
                        StatusCode::BAD_REQUEST,
                        format!("invalid id={}, expect uint32 number", car_id),
                    )
                }
            },
            None => {
                return mk_err_response(StatusCode::BAD_REQUEST, format!("expect id in url path"))
            }
        };

        match decode_request_body::<Car>(req).await {
            Ok(mut car) => {
                car.id = car_id;
                if car.year <= 0 {
                    return mk_err_response(
                        StatusCode::BAD_REQUEST,
                        "car year must be greater than 0",
                    );
                };
                match self.car_store.update_car(car) {
                    Ok(()) => mk_json_response("{}"),
                    Err(e) => Self::store_err_to_resp(e),
                }
            }
            Err(e) => mk_err_response(StatusCode::BAD_REQUEST, format!("invalid json input:{e}")),
        }
    }

    async fn delete_car(self, ctx: httputil::Context, _: Request<Incoming>) -> Response<BoxBody> {
        match ctx.vars.get("id") {
            Some(car_id) => {
                let id: u32 = match car_id.trim().parse() {
                    Ok(num) => num,
                    Err(_) => {
                        return mk_err_response(
                            StatusCode::BAD_REQUEST,
                            format!("invalid id={}, expect uint32 number", car_id),
                        )
                    }
                };
                match self.car_store.delete_car(id) {
                    Ok(()) => mk_json_response("{}"),
                    Err(e) => Self::store_err_to_resp(e),
                }
            }
            None => {
                return mk_err_response(StatusCode::BAD_REQUEST, format!("expect id in url path"))
            }
        }
    }

    async fn delete_all_cars(
        self,
        _: httputil::Context,
        _: Request<Incoming>,
    ) -> Response<BoxBody> {
        match self.car_store.delete_all_cars() {
            Ok(()) => mk_json_response("{}"),
            Err(e) => Self::store_err_to_resp(e),
        }
    }

    fn list_images(
        self,
        _: httputil::Context,
        _: Request<Incoming>,
    ) -> impl Future<Output = Response<BoxBody>> {
        async { ret_to_resp(ctl::list_images()) }
    }

    fn push_image(
        self,
        _: httputil::Context,
        r: Request<Incoming>,
    ) -> impl Future<Output = Response<BoxBody>> {
        async {
            #[derive(serde::Deserialize)]
            struct RequestPushImage {
                image: String,
            }
            match decode_request_body::<RequestPushImage>(r).await {
                Ok(img) => ret_to_resp(ctl::push_image(img.image)),
                Err(e) => {
                    mk_err_response(StatusCode::BAD_REQUEST, format!("invalid json input:{e}"))
                }
            }
        }
    }

    fn build_router() -> Router {
        fn add_route(
            mux: &mut HashMap<Method, matchit::Router<HandlerFn>>,
            path: &str,
            methed: Method,
            handler: httputil::BoxCloneHandler<Svc, Request<Incoming>, Response<BoxBody>>,
        ) -> () {
            mux.entry(methed)
                .or_default()
                .insert(path, handler.into())
                .unwrap();
        }

        let mut mux: HashMap<Method, matchit::Router<HandlerFn>> = Router::new();
        add_route(
            &mut mux,
            "/cars",
            Method::POST,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::create_car)),
        );
        add_route(
            &mut mux,
            "/cars/:id",
            Method::PUT,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::update_car)),
        );
        add_route(
            &mut mux,
            "/cars",
            Method::GET,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::get_car_list)),
        );
        add_route(
            &mut mux,
            "/cars/:id",
            Method::GET,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::get_car_by_id)),
        );
        add_route(
            &mut mux,
            "/cars",
            Method::DELETE,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::delete_all_cars)),
        );
        add_route(
            &mut mux,
            "/cars/:id",
            Method::DELETE,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::delete_car)),
        );

        add_route(
            &mut mux,
            "/ctl/images",
            Method::GET,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::list_images)),
        );
        add_route(
            &mut mux,
            "/ctl/images",
            Method::POST,
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::push_image)),
        );
        return mux;
    }
}

impl hyper::service::Service<Request<Incoming>> for Svc {
    type Response = Response<BoxBody>;
    type Error = GenericError;
    type Future = Pin<Box<dyn Future<Output = Result<Response<BoxBody>, GenericError>> + Send>>;

    fn call(&mut self, req: Request<Incoming>) -> Self::Future {
        Box::pin(route(self.mux.clone(), self.clone(), req))
    }
}

#[tokio::main]
async fn main() -> Result<(), GenericError> {
    pretty_env_logger::init();
    let addr = SocketAddr::from(([127, 0, 0, 1], 9100));
    let listener = TcpListener::bind(addr).await?;

    let carstore = match std::env::var("DB_TYPE") {
        // Ok(dbtyp) => match dbtyp.as_str() {
        //     "sqlite" => &SQLiteCarStore::new() as &dyn CarStore, // temporary value is freed at the end of this statement
        //     _ => &MemCarStore::init() as &dyn CarStore, // temporary value is freed at the end of this statement
        // },
        // Err(_) => &MemCarStore::init() as &dyn CarStore, //temporary value is freed at the end of this statement
        Ok(dbtyp) => match dbtyp.as_str() {
            "sqlite" => Box::new(SQLiteCarStore::new()) as Box<dyn CarStore + Send + Sync>,
            _ => Box::new(MemCarStore::init()) as Box<dyn CarStore + Send + Sync>,
        },
        Err(_) => Box::new(MemCarStore::init()) as Box<dyn CarStore + Send + Sync>,
    };
    let svc = Svc {
        // the size for values of type `dyn store::CarStore + Send + Sync` cannot be known at compilation time
        //   the trait `Sized` is not implemented for `dyn store::CarStore + Send + Sync`
        // car_store: std::sync::Arc::new(*carstore),
        //
        car_store: std::sync::Arc::from(carstore),
        mux: std::sync::Arc::new(Svc::build_router()),
    };
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let svc = svc.clone();
        // let mux = _mux.clone();
        // let handle_service = hyper::service::service_fn(move |req| {
        //     let f: Pin<Box<dyn Future<Output = Result<Response<BoxBody>>> + Send>> =
        //         Box::pin(route(mux.clone(), svc.clone(), req));
        //     f
        // });
        // this sucks:
        // higher-ranked lifetime error
        // could not prove `[async block@src/main.rs:380:28: 388:10]: Send`
        // https://github.com/rust-lang/rust/issues/102211
        // tokio::task::spawn(async move {
        //     if let Err(err) = http1::Builder::new()
        //         .serve_connection(stream, handle_service)
        //         .await
        //     {
        //         println!("Error serving connection: {:?}", err);
        //     }
        // });
        // so putting mux in Svc to solve it
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(stream, svc).await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

type HandlerFn =
    std::sync::Mutex<httputil::BoxCloneHandler<Svc, Request<Incoming>, Response<BoxBody>>>;
type Router = HashMap<Method, matchit::Router<HandlerFn>>;

fn route(
    mux: std::sync::Arc<Router>,
    s: Svc,
    req: Request<Incoming>,
) -> impl Future<Output = Result<Response<BoxBody>, GenericError>> + Send {
    async move {
        // find the subrouter for this request method
        let router = match mux.get(req.method()) {
            Some(router) => router,
            None => return Ok(mk_err_response(StatusCode::METHOD_NOT_ALLOWED, "")),
        };

        match router.at(req.uri().path()) {
            Ok(found) => {
                let mut ctx = httputil::Context {
                    vars: HashMap::new(),
                };
                for p in found.params.iter() {
                    ctx.vars.insert(p.0.to_owned(), p.1.to_owned());
                }
                // lock the service for a very short time, just to clone the service
                let _res = {
                    let mut ha = found.value.lock().unwrap().clone();
                    httputil::Handler::call(&mut ha, s, ctx, req).await
                };
                Ok(_res)
            }
            // if we there is no matching service, call the 404 handler
            Err(_) => Ok(mk_err_response(StatusCode::NOT_FOUND, "")),
        }
    }
}
