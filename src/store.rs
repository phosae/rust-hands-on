use serde::{Deserialize, Serialize};
use std::{
    mem,
    sync::{atomic::AtomicU32, RwLock},
};

#[derive(PartialEq, Debug)]
#[allow(dead_code)]
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
