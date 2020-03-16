use calendar_app_lib::config::Config;
use gcal_lib::gcal_instance::GCalendarInstance;

fn main() {
    let config = Config::init_config().unwrap();
    let gcal_inst = GCalendarInstance::new(
        &config.gcal_token_path,
        &config.gcal_secret_file,
        "ddboline@gmail.com",
    );
    let result = gcal_inst.list_gcal_calendars().unwrap();
    for cal in result {
        println!(
            "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n\n",
            cal.id.unwrap(),
            cal.summary.unwrap(),
            cal.description.as_ref().map_or("", String::as_str),
            cal.time_zone.as_ref().map_or("", String::as_str),
            cal.location.as_ref().map_or("", String::as_str),
        );
    }
}
