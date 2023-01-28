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
type Result<T> = std::result::Result<T, GenericError>;
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

async fn decode_request_body<T: DeserializeOwned>(
    req: Request<Incoming>,
) -> std::result::Result<T, String> {
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

    fn get_car_list(&self) -> Response<BoxBody> {
        match self.car_store.get_all_cars() {
            Ok(cars) => mk_json_response(&cars),
            Err(e) => Svc::store_err_to_resp(e),
        }
    }

    fn get_car_by_id(&self, id: u32) -> Response<BoxBody> {
        match self.car_store.get_car(id) {
            Ok(car) => mk_json_response(&car),
            Err(store_err) => Self::store_err_to_resp(store_err),
        }
    }

    fn create_car(&self, new_car: Car) -> Response<BoxBody> {
        if new_car.year <= 0 {
            return mk_err_response(StatusCode::BAD_REQUEST, "car year must be greater than 0");
        }
        match self
            .car_store
            .create_car(new_car.brand, new_car.model, new_car.year)
        {
            Ok(nid) => mk_json_response(&json!({ "id": nid }).to_string()),
            Err(e) => Svc::store_err_to_resp(e),
        }
    }

    fn update_car(&self, car: Car) -> Response<BoxBody> {
        if car.year <= 0 {
            return mk_err_response(StatusCode::BAD_REQUEST, "car year must be greater than 0");
        }
        match self.car_store.update_car(car) {
            Ok(()) => mk_json_response("{}"),
            Err(e) => Self::store_err_to_resp(e),
        }
    }

    fn delete_car(&self, id: u32) -> Response<BoxBody> {
        match self.car_store.delete_car(id) {
            Ok(()) => mk_json_response("{}"),
            Err(e) => Self::store_err_to_resp(e),
        }
    }

    fn delete_all_cars(&self) -> Response<BoxBody> {
        match self.car_store.delete_all_cars() {
            Ok(()) => mk_json_response("{}"),
            Err(e) => Self::store_err_to_resp(e),
        }
    }

    #[allow(dead_code)]
    fn list_images(
        self,
        _: httputil::Context,
        _: Request<Incoming>,
    ) -> impl Future<Output = Response<BoxBody>> {
        async { ret_to_resp(ctl::list_images()) }
    }

    #[allow(dead_code)]
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
}

impl hyper::service::Service<Request<Incoming>> for Svc {
    type Response = Response<BoxBody>;
    type Error = GenericError;
    type Future = Pin<Box<dyn Future<Output = Result<Response<BoxBody>>> + Send>>;

    fn call(&mut self, req: Request<Incoming>) -> Self::Future {
        let path = req.uri().path().to_owned();
        let path_segments: Vec<&str> = path.split("/").collect::<Vec<&str>>();
        let base_path = path_segments[1];
        match (req.method(), base_path) {
            (&Method::GET, "cars") => {
                if path_segments.len() <= 2 {
                    let res = self.get_car_list();
                    return Box::pin(async { Ok(res) });
                } else {
                    let car_id = path_segments[2];

                    if car_id.trim().is_empty() {
                        let res = self.get_car_list();
                        return Box::pin(async { Ok(res) });
                    } else {
                        let id: u32 = match car_id.trim().parse() {
                            Ok(num) => num,
                            Err(_) => {
                                let res = mk_err_response(
                                    StatusCode::BAD_REQUEST,
                                    format!("invalid id={}, expect uint32 number", car_id),
                                );
                                return Box::pin(async { Ok(res) });
                            }
                        };
                        let res = self.get_car_by_id(id);
                        return Box::pin(async { Ok(res) });
                    }
                }
            }

            (&Method::DELETE, "cars") => {
                if path_segments.len() <= 2 {
                    let res = self.delete_all_cars();
                    return Box::pin(async { Ok(res) });
                } else {
                    let car_id = path_segments[2];

                    if car_id.trim().is_empty() {
                        let res = self.delete_all_cars();
                        return Box::pin(async { Ok(res) });
                    } else {
                        let id: u32 = match car_id.trim().parse() {
                            Ok(num) => num,
                            Err(_) => {
                                let res = mk_err_response(
                                    StatusCode::BAD_REQUEST,
                                    format!("invalid id={}, expect uint32 number", car_id),
                                );
                                return Box::pin(async { Ok(res) });
                            }
                        };
                        let res = self.delete_car(id);
                        return Box::pin(async { Ok(res) });
                    }
                }
            }

            (&Method::POST, "cars") => {
                let svc = self.clone();
                let res = async move {
                    match decode_request_body::<Car>(req).await {
                        // the most tricky part is:
                        //   we can't simply use self.create_car here, as Service trait's call fn will return a Future to caller.
                        // and the compiler will complain:
                        //
                        //   fn call(&mut self, req: Request<Incoming>) -> Self::Future {
                        //          - let's call the lifetime of this reference `'1`
                        //               return Box::pin(res);
                        //                                    returning this value requires that `'1` must outlive `'static`
                        //
                        // So i guess: once Svc is dead, what self pointed to is undefined
                        // see: https://users.rust-lang.org/t/how-to-call-async-static-service-methods-in-hyper/66019
                        //
                        // Ok(new_car) => Ok(self.create_car(new_car)),
                        Ok(new_car) => Ok(svc.create_car(new_car)),
                        Err(e) => Ok(mk_err_response(
                            StatusCode::BAD_REQUEST,
                            format!("invalid json input:{e}"),
                        )),
                    }
                };
                return Box::pin(res);
            }

            (&Method::PUT, "cars") => {
                if path_segments.len() <= 2 {
                    return Box::pin(async { Ok(mk_err_response(StatusCode::NOT_FOUND, "")) });
                }

                let car_id_str = path_segments[2];
                let car_id = match car_id_str.trim().parse::<u32>() {
                    Ok(num) => num,
                    Err(_) => {
                        let res = mk_err_response(
                            StatusCode::BAD_REQUEST,
                            format!("invalid id={}, expect uint32 number", car_id_str),
                        );
                        return Box::pin(async { Ok(res) });
                    }
                };

                let svc = self.clone();
                let res = async move {
                    match decode_request_body::<Car>(req).await {
                        Ok(mut car) => {
                            car.id = car_id;
                            Ok(svc.update_car(car))
                        }
                        Err(e) => Ok(mk_err_response(
                            StatusCode::BAD_REQUEST,
                            format!("invalid json input:{e}"),
                        )),
                    }
                };
                return Box::pin(res);
            }

            (&Method::GET, "images") => {
                let svc = self.clone();
                let res = async move {
                    let aert = svc
                        .list_images(
                            httputil::Context {
                                vars: std::collections::HashMap::from([(
                                    "k".to_owned(),
                                    "v".to_owned(),
                                )]),
                            },
                            req,
                        )
                        .await;
                    Ok(aert)
                };
                return Box::pin(res);
            }

            // Return the 404 Not Found for other routes.
            //_ => Box::pin(async { Ok(mk_err_response(StatusCode::NOT_FOUND, "")) }),
            _ => Box::pin(Box::pin(route(self.mux.clone(), self.clone(), req))),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
        mux: std::sync::Arc::new(build_router()),
    };
    let router = build_router();
    let _mux = std::sync::Arc::new(router);
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
) -> impl Future<Output = Result<Response<BoxBody>>> + Send {
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

fn build_router() -> Router {
    let mut mux = Router::new();
    mux.entry(Method::GET)
        .or_default()
        .insert(
            "/images",
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::list_images)).into(),
        )
        .unwrap();
    mux.entry(Method::POST)
        .or_default()
        .insert(
            "/images",
            httputil::BoxCloneHandler::new(httputil::handler_fn(Svc::list_images)).into(),
        )
        .unwrap();
    return mux;
}
