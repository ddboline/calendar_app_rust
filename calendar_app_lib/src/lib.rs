#[macro_use]
extern crate diesel;

pub mod calendar;
pub mod calendar_sync;
pub mod config;
pub mod latitude;
pub mod longitude;
pub mod models;
pub mod parse_hashnyc;
pub mod pgpool;
pub mod schema;
pub mod timezone;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
