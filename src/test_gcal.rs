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
    println!("{:?}", result);
}
