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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

pub trait CarStore {
    fn create_car(&self, brand: String, model: String, year: u16) -> Result<u32, StoreError>;
    fn update_car(&self, car: Car) -> Result<(), StoreError>;
    fn get_car(&self, id: u32) -> Result<Car, StoreError>;
    fn get_all_cars(&self) -> Result<Vec<Car>, StoreError>;
    fn delete_car(&self, id: u32) -> Result<(), StoreError>;
    fn delete_all_cars(&self) -> Result<(), StoreError>;
}

pub struct MemCarStore {
    cars: RwLock<Vec<Car>>,
    next_id: AtomicU32,
}

impl MemCarStore {
    pub fn init() -> MemCarStore {
        MemCarStore {
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
}

impl CarStore for MemCarStore {
    fn create_car(
        &self,
        brand: String,
        model: String,
        year: u16,
    ) -> std::result::Result<u32, StoreError> {
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
        return Ok(id);
    }

    fn update_car(&self, car: Car) -> Result<(), StoreError> {
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

    fn get_car(&self, id: u32) -> Result<Car, StoreError> {
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

    fn get_all_cars(&self) -> std::result::Result<Vec<Car>, StoreError> {
        let reader = self.cars.read().unwrap();
        return Ok(reader.clone());
    }

    fn delete_car(&self, id: u32) -> Result<(), StoreError> {
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

    fn delete_all_cars(&self) -> Result<(), StoreError> {
        let mut writer = self.cars.write().unwrap();
        _ = mem::replace(&mut *writer, vec![]);
        self.next_id.store(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

use rusqlite::{Connection, Result};
pub struct SQLiteCarStore;

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

impl SQLiteCarStore {
    fn dbconn() -> Result<Connection, StoreError> {
        Ok(Connection::open("cars.db")?)
    }

    pub fn new() -> SQLiteCarStore {
        let conn = Self::dbconn().unwrap();
        conn.execute(
            "create table if not exists cars (
                 id integer primary key autoincrement,
                 brand text not null,
                 model text not null,
                 year integer
             )",
            (),
        )
        .unwrap();
        SQLiteCarStore {}
    }
}

impl CarStore for SQLiteCarStore {
    fn create_car(
        &self,
        brand: String,
        model: String,
        year: u16,
    ) -> std::result::Result<u32, StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        conn.execute(
            "INSERT INTO cars (brand,model,year) values (?1,?2,?3)",
            &[&brand, &model, &year.to_string()],
        )?;
        Ok(conn.last_insert_rowid().try_into().unwrap())
    }

    fn update_car(&self, car: Car) -> Result<(), StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        conn.execute(
            "UPDATE cars SET brand=?1,mode=?2,year=?3 WHERE id=?4)",
            &[
                &car.brand,
                &car.model,
                &car.year.to_string(),
                &car.id.to_string(),
            ],
        )?;
        Ok(())
    }

    fn get_car(&self, id: u32) -> Result<Car, StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        let mut stmt = conn.prepare("SELECT id,brand,model,year FROM cars where id=?")?;
        let mut car_iter = stmt.query_map([id], |row| {
            Ok(Car {
                id: row.get(0)?,
                brand: row.get(1)?,
                model: row.get(2)?,
                year: row.get(3)?,
            })
        })?;
        match car_iter.find_map(|maycar| {
            if maycar.is_ok() {
                let car = maycar.unwrap();
                if car.id == id {
                    Some(car)
                } else {
                    None
                }
            } else {
                None
            }
        }) {
            Some(car) => Ok(car),
            None => Err(StoreError::NotFound(format!(
                "car with id={} not found",
                id
            ))),
        }
    }

    fn get_all_cars(&self) -> std::result::Result<Vec<Car>, StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        let mut stmt = conn.prepare("SELECT id,brand,model,year FROM cars")?;
        let car_iter = stmt.query_map([], |row| {
            Ok(Car {
                id: row.get(0)?,
                brand: row.get(1)?,
                model: row.get(2)?,
                year: row.get(3)?,
            })
        })?;
        Ok(car_iter
            .filter(|may| may.is_ok())
            .map(|may| may.unwrap())
            .collect::<Vec<Car>>())
    }

    fn delete_car(&self, id: u32) -> Result<(), StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        conn.execute("DELETE FROM cars WHERE id=?", &[&id.to_string()])?;
        Ok(())
    }

    fn delete_all_cars(&self) -> Result<(), StoreError> {
        let conn = SQLiteCarStore::dbconn()?;
        conn.execute("DELETE FROM cars", ())?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_create_car() {
        let sqlcars = SQLiteCarStore::new();
        let nid = sqlcars
            .create_car("BYD".to_owned(), "Han".to_owned(), 2020)
            .expect("should return new row id in cars table");
        sqlcars
            .get_car(nid)
            .expect("should return the new created car");
        sqlcars.get_all_cars().expect("list cars should be ok");
        sqlcars.delete_car(nid).expect("delete the new created car");
    }

    #[test]
    fn test_delete_car() {
        let sqlcars = SQLiteCarStore::new();
        println!(
            "{:?}{:?}",
            sqlcars.create_car("BYD".to_owned(), "Han".to_owned(), 2020),
            sqlcars.create_car("Tesla".to_owned(), "Mode X".to_owned(), 2015)
        );
        let cars = sqlcars.get_all_cars().expect("list car should be ok");
        assert!(cars.len() >= 2);
        sqlcars
            .delete_all_cars()
            .expect("delete all cars should be ok");
    }
}
