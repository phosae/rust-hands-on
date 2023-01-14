# v2
## add store.rs
In this file I import CarStore to manage car state. Car state is wrapped by RwLock to support multi-thread context.
```
pub struct CarStore {
    cars: RwLock<Vec<Car>>,
    next_id: AtomicU32,
}
```
in Golang it will look like this
```
type CarStore struct {
	sync.Mutex

	cars  []Car
	nextId int
}
```
and I learn some thing about Lock in Rust, when write/read was call on RwLock
1. try to get the mutex
2. create new Object RwLockWriteGuard pointed to the RwLock Object
3. then the rest code block of update_car is guarded by RwLockWriteGuard
4. when update_car returned, RwLockWriteGuard on stack was free, cause the mutex auto released

internal here is much like RAII in c++

```
pub fn update_car(&self, car: Car) -> Result<(), StoreError> {
    let mut writer: std::sync::RwLockWriteGuard<Vec<Car>> = self.cars.write().unwrap();
    match writer.iter_mut().find(|ocar| ocar.id == car.id) { -----
        Some(ocar) => {                                          |
            ocar.brand = car.brand;                              |
            ocar.model = car.model;                              |
            ocar.year = car.year;                                |
            Ok(())                                               |---------------> code block guarded by RwLockWriteGuard
        }                                                        |
        None => Err(StoreError::NotFound(format!(                |
            "car with id={} not found",                          |
            car.id                                               |
        ))),                                                     |
    } ------------------------------------------------------------
}
```

## change in main.rs

- add generic async fn decode_request_body to decode request body's json bytes to Rust structs
- add struct Svc, it
  - hold car_store, wrapped it with std::sync::Arc, so it's ref can clone to multi threads
  - write HTTP logic ontop CarStore
  - impl hyper hyper::service::Service trait, then it can work as HTTP router

init Svc and each connection got a ref to it(maybe just CarStore)
```
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
```

the most tricky part is, when I need to decode the request body, as it must be done in async block, but 

I can't simply use self in async block(so I clone the svc and move to async block), as Service trait's call fn will return a Future to caller.
and the compiler will complain:
```
fn call(&mut self, req: Request<Incoming>) -> Self::Future {
        - let's call the lifetime of this reference `'1`
            return Box::pin(res);
                                returning this value requires that `'1` must outlive `'static`
```
So i guess: once Svc is dead, what self pointed to is undefined
see: https://users.rust-lang.org/t/how-to-call-async-static-service-methods-in-hyper/66019
                        
```
(&Method::POST, "cars") => {
    let svc = self.clone();
    let res = async move {
        match decode_request_body::<Car>(req).await {
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
```

## other
- add logging facade `log = "0.4.17"`

## still don't understand
- Box
- Pin
- Box<dyn std::error::Error + Send + Sync>
- ...