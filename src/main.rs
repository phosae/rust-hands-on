#![deny(warnings)]

use bytes::{Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;

const INTERNAL_SERVER_ERROR: &str = "Internal Server Error";
static NOTFOUND: &[u8] = b"Not Found";
type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn mk_404() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(full(NOTFOUND))
        .unwrap()
}

fn mk_400<T: Into<Bytes>>(msg: T) -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(full(msg))
        .unwrap()
}

#[derive(Serialize, Deserialize)]
struct Car {
    #[serde(default = "default_car_id")] // https://serde.rs/field-attrs.html
    id: String,
    brand: String,
    model: String,
    year: u16,
}

fn default_car_id() -> String {
    let mut random = rand::thread_rng();
    let car_id: u8 = random.gen();
    return car_id.to_string();
}

fn get_car_list() -> Response<BoxBody> {
    let cars: [Car; 3] = [
        Car {
            id: "1".to_owned(),
            brand: "Ford".to_owned(),
            model: "Bronco".to_owned(),
            year: 2022,
        },
        Car {
            id: "2".to_owned(),
            brand: "Hyundai".to_owned(),
            model: "Santa Fe".to_owned(),
            year: 2010,
        },
        Car {
            id: "3".to_owned(),
            brand: "Dodge".to_owned(),
            model: "Challenger".to_owned(),
            year: 2015,
        },
    ];

    match serde_json::to_string(&cars) {
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

fn get_car_by_id(car_id: &String) -> Response<BoxBody> {
    let cars: [Car; 3] = [
        Car {
            id: "1".to_owned(),
            brand: "Ford".to_owned(),
            model: "Bronco".to_owned(),
            year: 2022,
        },
        Car {
            id: "2".to_owned(),
            brand: "Hyundai".to_owned(),
            model: "Santa Fe".to_owned(),
            year: 2010,
        },
        Car {
            id: "3".to_owned(),
            brand: "Dodge".to_owned(),
            model: "Challenger".to_owned(),
            year: 2015,
        },
    ];

    let car_index_option = cars.iter().position(|x| &x.id == car_id);

    if car_index_option.is_none() {
        return mk_404();
    }

    let car = &cars[car_index_option.unwrap()];

    match serde_json::to_string(car) {
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

async fn create_car(req: Request<Incoming>) -> Result<Response<BoxBody>> {
    fn do_create_car(new_car: Car) -> Result<Response<BoxBody>> {
        if new_car.year <= 0 {
            return Ok(mk_400("car year must be greater than 0"));
        }

        let json = serde_json::to_string(&new_car)?;
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(full(json))?;
        Ok(response)
    }

    // get the buffer from the request body
    let buffer = req.collect().await?.aggregate();
    //let mut new_car: serde_json::Value = serde_json::from_reader(buffer.reader())?;
    let mut de = serde_json::Deserializer::from_reader(buffer.reader());
    match Car::deserialize(&mut de) {
        Ok(new_car) => do_create_car(new_car),
        Err(e) => Ok(mk_400(format!("invalid json input:{e}"))),
    }
}

async fn cars_handler(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody>> {
    let path = req.uri().path().to_owned();
    let path_segments = path.split("/").collect::<Vec<&str>>();
    let base_path = path_segments[1];

    match (req.method(), base_path) {
        (&Method::GET, "cars") => {
            if path_segments.len() <= 2 {
                let res = get_car_list();
                return Ok(res);
            }

            let car_id = path_segments[2];

            if car_id.trim().is_empty() {
                let res = get_car_list();
                return Ok(res);
            } else {
                let res = get_car_by_id(&car_id.to_string());
                Ok(res)
            }
        }

        (&Method::POST, "cars") => create_car(req).await,

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 9100));

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(stream, service_fn(cars_handler))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
