#![deny(warnings)]
mod store;

use bytes::{Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header;
use hyper::server::conn::http1;
use hyper::{Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use store::{Car, CarStore, StoreError};
use tokio::net::TcpListener;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

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

#[derive(Clone)]
struct Svc {
    car_store: std::sync::Arc<CarStore>,
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
        let cars = self.car_store.get_all_cars();
        mk_json_response(&cars)
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
        let nid = self
            .car_store
            .create_car(new_car.brand, new_car.model, new_car.year);
        let json = json!({ "id": nid }).to_string();
        mk_json_response(&json)
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

            // Return the 404 Not Found for other routes.
            _ => Box::pin(async { Ok(mk_err_response(StatusCode::NOT_FOUND, "")) }),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    let addr = SocketAddr::from(([127, 0, 0, 1], 9100));
    let listener = TcpListener::bind(addr).await?;
    let svc = Svc {
        car_store: std::sync::Arc::new(CarStore::init()),
    };
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let svc = svc.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(stream, svc).await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
