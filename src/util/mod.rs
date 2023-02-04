mod babe_svc_ref;
pub mod http;
mod lifetime_handler_sucks;
mod mock_tower_svc;

#[allow(dead_code)]
pub fn type_of<T>(_:&T) -> &str {
    std::any::type_name::<T>()
}
