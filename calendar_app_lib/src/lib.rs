#[macro_use]
extern crate diesel;

pub mod config;
pub mod models;
pub mod pgpool;
pub mod schema;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
