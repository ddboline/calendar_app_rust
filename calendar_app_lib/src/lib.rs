#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::manual_range_contains)]

#[macro_use]
extern crate diesel;

pub mod calendar;
pub mod calendar_cli_opts;
pub mod calendar_sync;
pub mod config;
pub mod latitude;
pub mod longitude;
pub mod models;
pub mod parse_hashnyc;
pub mod parse_nycruns;
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
