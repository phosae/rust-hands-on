#![deny(warnings)]

use bytes::{Buf, Bytes};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::Deserialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use store::{Car, CarStore};

fn mk_404() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(StatusCode::NOT_FOUND.as_str()))
        .unwrap()
}

fn mk_400<T: Into<Bytes>>(msg: T) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(msg.into()))
        .unwrap()
}

fn mk_500() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(StatusCode::INTERNAL_SERVER_ERROR.as_str()))
        .unwrap()
}

// CORS headers
fn response_build(body: &str) -> Response<Body> {
    Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "api,Keep-Alive,User-Agent,Content-Type",
        )
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn parse_id(sid: &str) -> Option<u32> {
    match sid.trim().parse() {
        Ok(num) => Some(num),
        Err(_) => None,
    }
}

async fn cars_handler(
    store: std::sync::Arc<CarStore>,
    req: Request<Body>,
) -> Result<Response<Body>, anyhow::Error> {
    let path = req.uri().path().to_owned();
    let path_segments = path.split("/").collect::<Vec<&str>>();
    let base_path = path_segments[1];

    match (req.method(), base_path) {
        (&Method::GET, "cars") => {
            fn listcars(store: std::sync::Arc<CarStore>) -> Response<Body> {
                let cars = store.get_all_cars();
                match serde_json::to_string(&cars) {
                    Ok(json) => response_build(json.as_str()),
                    Err(_) => mk_500(),
                }
            }
            if path_segments.len() <= 2 {
                return Ok(listcars(store));
            }

            let car_id = path_segments[2];

            if car_id.trim().is_empty() {
                return Ok(listcars(store));
            } else {
                match parse_id(car_id) {
                    Some(id) => match store.get_car(id) {
                        Ok(car) => Ok(response_build(serde_json::to_string(&car)?.as_str())),
                        Err(e) => Ok(e.into()),
                    },
                    None => {
                        return Ok(mk_400(format!(
                            "invalid id={}, expect uint32 number",
                            car_id
                        )))
                    }
                }
            }
        }

        (&Method::DELETE, "cars") => {
            fn deletecars(store: std::sync::Arc<CarStore>) -> Response<Body> {
                match store.delete_all_cars() {
                    Ok(_) => response_build("{}"),
                    Err(_) => mk_500(),
                }
            }
            if path_segments.len() <= 2 {
                return Ok(deletecars(store));
            }

            let car_id = path_segments[2];

            if car_id.trim().is_empty() {
                return Ok(deletecars(store));
            } else {
                match parse_id(car_id) {
                    Some(id) => match store.delete_car(id) {
                        Ok(_) => Ok(response_build("")),
                        Err(e) => Ok(e.into()),
                    },
                    None => {
                        return Ok(mk_400(format!(
                            "invalid id={}, expect uint32 number",
                            car_id
                        )))
                    }
                }
            }
        }

        (&Method::POST, "cars") => {
            let buffer = hyper::body::to_bytes(req).await?;
            let mut de = serde_json::Deserializer::from_reader(buffer.reader());
            match Car::deserialize(&mut de) {
                Ok(new_car) => Ok(response_build(
                    serde_json::to_string(&store.create_car(
                        new_car.brand,
                        new_car.model,
                        new_car.year,
                    ))?
                    .as_str(),
                )),
                Err(e) => Ok(mk_400(format!("invalid json input:{e}"))),
            }
        }

        (&Method::PUT, "cars") => {
            if path_segments.len() <= 2 {
                return Ok(mk_404());
            }
            let car_id_str = path_segments[2];
            match parse_id(car_id_str) {
                Some(id) => {
                    let buffer = hyper::body::to_bytes(req).await?;
                    let mut de = serde_json::Deserializer::from_reader(buffer.reader());
                    match Car::deserialize(&mut de) {
                        Ok(new_car) => {
                            match store.update_car(Car {
                                id,
                                brand: new_car.brand,
                                model: new_car.model,
                                year: new_car.year,
                            }) {
                                Ok(_) => Ok(response_build("")),
                                Err(e) => Ok(e.into()),
                            }
                        }
                        Err(e) => Ok(mk_400(format!("invalid json input:{e}"))),
                    }
                }
                None => Ok(mk_400(format!(
                    "invalid id={}, expect uint32 number",
                    car_id_str
                ))),
            }
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 9100));
    println!("Listening on http://{}", addr);
    let car_store = std::sync::Arc::new(CarStore::init());
    let make_svc = make_service_fn(|_| {
        let car_store = car_store.clone();
        async move { Ok::<_, Infallible>(service_fn(move |req| cars_handler(car_store.clone(), req))) }
    });
    let server = Server::bind(&addr).serve(make_svc);
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
    Ok(())

    // It seems we can't use TcpListener in hyper_wasi
    // but only `Server::bind(&addr).serve(service_fn(svc))`, which was removed since hyper 1.0.0-rc...

    // let listener = TcpListener::bind(addr).await?;
    // loop {
    //     let (stream, _) = listener.accept().await?;

    //     tokio::task::spawn(async move {
    //         if let Err(err) = http1::Builder::new()
    //             .serve_connection(stream, service_fn(cars_handler))
    //             .await
    //         {
    //             println!("Error serving connection: {:?}", err);
    //         }
    //     });
    // }
}

impl Into<Response<Body>> for store::StoreError {
    fn into(self) -> Response<Body> {
        match self {
            store::StoreError::NotFound(_) => mk_404(),
            store::StoreError::Internal(_) => mk_500(),
        }
    }
}

pub mod store {
    use serde::{Deserialize, Serialize};
    use std::{
        mem,
        sync::{atomic::AtomicU32, RwLock},
    };

    #[derive(PartialEq, Debug)]
    pub enum StoreError {
        NotFound(String),
        Internal(String),
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Car {
        #[serde(default = "default_car_id")] // https://serde.rs/field-attrs.html
        pub id: u32,
        pub brand: String,
        pub model: String,
        pub year: u16,
    }

    fn default_car_id() -> u32 {
        0
    }

    pub struct CarStore {
        cars: RwLock<Vec<Car>>,
        next_id: AtomicU32,
    }

    impl CarStore {
        pub fn init() -> CarStore {
            CarStore {
                cars: RwLock::new(vec![
                    Car {
                        id: 1,
                        brand: "Ford".to_owned(),
                        model: "Bronco".to_owned(),
                        year: 2022,
                    },
                    Car {
                        id: 2,
                        brand: "Hyundai".to_owned(),
                        model: "Santa Fe".to_owned(),
                        year: 2010,
                    },
                    Car {
                        id: 3,
                        brand: "Dodge".to_owned(),
                        model: "Challenger".to_owned(),
                        year: 2015,
                    },
                ]),
                next_id: AtomicU32::new(4),
            }
        }

        pub fn create_car(&self, brand: String, model: String, year: u16) -> u32 {
            let mut writer = self.cars.write().unwrap();
            let id = self
                .next_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            writer.push(Car {
                id,
                brand,
                model,
                year,
            });
            return id;
        }

        pub fn update_car(&self, car: Car) -> Result<(), StoreError> {
            let mut writer: std::sync::RwLockWriteGuard<Vec<Car>> = self.cars.write().unwrap();
            match writer.iter_mut().find(|ocar| ocar.id == car.id) {
                Some(ocar) => {
                    ocar.brand = car.brand;
                    ocar.model = car.model;
                    ocar.year = car.year;
                    Ok(())
                }
                None => Err(StoreError::NotFound(format!(
                    "car with id={} not found",
                    car.id
                ))),
            }
        }

        pub fn get_car(&self, id: u32) -> Result<Car, StoreError> {
            let reader = self.cars.read().unwrap();
            let car = reader.iter().find(|&car| car.id == id).cloned();
            match car {
                Some(car) => Ok(car),
                None => Err(StoreError::NotFound(format!(
                    "car with id={} not found",
                    id
                ))),
            }
        }

        pub fn get_all_cars(&self) -> Vec<Car> {
            let reader = self.cars.read().unwrap();
            return reader.clone();
        }

        pub fn delete_car(&self, id: u32) -> Result<(), StoreError> {
            let mut writer = self.cars.write().unwrap();
            match writer.iter().position(|car| car.id == id) {
                None => Err(StoreError::NotFound(format!(
                    "car with id={} not found",
                    id
                ))),
                Some(idx) => {
                    writer.remove(idx);
                    Ok(())
                }
            }
        }

        pub fn delete_all_cars(&self) -> Result<(), StoreError> {
            let mut writer = self.cars.write().unwrap();
            _ = mem::replace(&mut *writer, vec![]);
            self.next_id.store(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }
}
