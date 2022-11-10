use dioxus::prelude::{
    Scope, Element, rsx, dioxus_elements,
    LazyNodes, NodeFactory, VNode,
    format_args_f, inline_props,
    Props, VirtualDom
};
use time::{macros::format_description};
use std::collections::HashMap;
use stack_string::{StackString, format_sstr};
use url::Url;
use itertools::Itertools;

use calendar_app_lib::{
    calendar::{Calendar, Event},
    get_default_or_local_time,
    config::Config,
};

pub fn index_body() -> String {
    let mut app = VirtualDom::new_with_props(
        index_element,
        index_elementProps {},
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}

#[inline_props]
fn index_element(cx: Scope) -> Element {
    cx.render(rsx! {
        head {
            style {[include_str!("../../templates/style.css")]},
        },
        body {
            br {
                input {
                    "type": "button",
                    name: "display_agenda",
                    value: "Agenda",
                    "onclick": "displayAgenda();",
                },
                input {
                    "type": "button",
                    name: "sync",
                    value: "Sync",
                    "onclick": "syncCalendars();",
                },
                input {
                    "type": "button",
                    name: "list_calendars",
                    value: "List Calendars",
                    "onclick": "listCalendars();",
                },
                button {
                    name: "garminconnectoutput",
                    id: "garminconnectoutput",
                    "&nbsp;",
                }
            }
            article {
                id: "main_article",
                "&nbsp;",
            },
            article {
                id: "sub_article",
                "&nbsp;",
            }
            script {
                "language": "JavaScript",
                "type": "text/javascript",
                [include_str!("../../templates/scripts.js")],
            }
        }
    })
}

pub fn agenda_body(
    calendar_map: HashMap<StackString, Calendar>,
    events: Vec<Event>,
    config: Config,
) -> String {
    let mut app = VirtualDom::new_with_props(
        agenda_element,
        agenda_elementProps {
            calendar_map,
            events,
            config,
        },
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}


#[inline_props]
fn agenda_element(
    cx: Scope,
    calendar_map: HashMap<StackString, Calendar>,
    events: Vec<Event>,
    config: Config,
) -> Element {
    cx.render(rsx! {
        table {
            "border": "1",
            class: "dataframe",
            thead {
                th {"Calendar"},
                th {"Event"},
                th {"Start Time"},
            },
            tbody {
                events.iter().enumerate().filter_map(|(idx, event)| {
                    let cal = match calendar_map.get(&event.gcal_id) {
                        Some(cal) => cal,
                        None => return None,
                    };
                    let calendar_name = cal.gcal_name.as_ref().unwrap_or(&cal.name);
                    let delete = if cal.edit {
                        let event_id = &event.event_id;
                        let gcal_id = &event.gcal_id;
                        Some(rsx! {
                            input {
                                "type": "button",
                                name: "delete_event",
                                value: "Delete",
                                "onclick": "deleteEventAgenda('{gcal_id}', '{event_id}')",
                            }
                        })
                    } else {
                        None
                    };
                    let start_time = get_default_or_local_time(event.start_time.into(), &config);
                    let calendar_name = calendar_name;
                    let cal_name = &cal.name;
                    let gcal_id = &event.gcal_id;
                    let event_id = &event.event_id;
                    let event_name = &event.name;
                    Some(rsx! {
                        tr {
                            key: "event-key-{idx}",
                            "text-style": "center",
                            td {
                                input {
                                    "type": "button",
                                    name: "list_events",
                                    value: "{calendar_name}",
                                    "onclick": "listEvents('{cal_name}')",
                                },
                            },
                            td {
                                input {
                                    "type": "button",
                                    name: "event_detail",
                                    value: "{event_name}",
                                    "onclick": "eventDetail('{gcal_id}', '{event_id}')",
                                }
                            },
                            td {"{start_time}"},
                            td {delete},
                        }
                    })
                })
            }
        }
    })
}

pub fn list_calendars_body(
    calendars: Vec<Calendar>,
) -> String {
    let mut app = VirtualDom::new_with_props(
        list_calendars_element,
        list_calendars_elementProps {
            calendars,
        },
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}

#[inline_props]
fn list_calendars_element(
    cx: Scope,
    calendars: Vec<Calendar>,
) -> Element {
    cx.render(rsx! {
        table {
            "border": "1",
            class: "dataframe",
            thead {
                th {"Calendar"},
                th {"Description"},
                th {},
                th {
                    input {
                        "type": "button",
                        name: "sync_all",
                        value: "Full Sync",
                        "onclick": "syncCalendarsFull();",
                    }
                }
            },
            tbody {
                calendars.iter().enumerate().map(|(idx, calendar)| {
                    let gcal_id = &calendar.gcal_id;
                    let create_event = if calendar.edit {
                        Some(rsx! {
                            input {
                                "type": "button",
                                name: "create_event",
                                value: "Create Event",
                                "onclick": "buildEvent('{gcal_id}')",
                            }
                        })
                    } else {
                        None
                    };
                    let make_visible = if calendar.display {
                        rsx! {
                            input {
                                "type": "button",
                                name: "hide_calendar",
                                value: "Hide",
                                "onclick": "calendarDisplay('{gcal_id}', false)",
                            }
                        }
                    } else {
                        rsx! {
                            input {
                                "type": "button",
                                name: "show_calendar",
                                value: "Show",
                                "onclick": "calendarDisplay('{gcal_id}', true)",
                            }                            
                        }
                    };
                    let calendar_name = &calendar.name;
                    let description = calendar.description.as_ref().map_or_else(|| "", StackString::as_str);
                    rsx !{
                        tr {
                            key: "calendar-key-{idx}",
                            "text-style": "center",
                            td {
                                input {
                                    "type": "button",
                                    name: "list_events",
                                    value: "{calendar_name}",
                                    "onclick": "calendarDisplay('{gcal_id}', false)",
                                }
                            },
                            td {"{description}"},
                            td {make_visible},
                            td {create_event},
                        }
                    }
                })
            }
        }
    })
}

pub fn list_events_body(
    calendar: Calendar,
    events: Vec<Event>,
    config: Config,
) -> String {
    let mut app = VirtualDom::new_with_props(
        list_events_element,
        list_events_elementProps {
            calendar,
            events,
            config,
        },
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}


#[inline_props]
fn list_events_element(
    cx: Scope,
    calendar: Calendar,
    events: Vec<Event>,
    config: Config,
) -> Element {
    let gcal_id = &calendar.gcal_id;
    cx.render(rsx! {
        table {
            "border": "1",
            class: "dataframe",
            thead {
                th {"Event"},
                th {"Start Time"},
                th {"End Time"},
                th {
                    input {
                        "type": "button",
                        name: "create_event",
                        value: "Create Event",
                        "onclick": "buildEvent('{gcal_id}')",
                    }
                }
            },
            tbody {
                events.iter().enumerate().map(|(idx, event)| {

                    let delete = if calendar.edit {
                        let gcal_id = &event.gcal_id;
                        let event_id = &event.event_id;
                        let calendar_name = &calendar.name;
                        Some(rsx! {
                            input {
                                "type": "button",
                                name: "delete_event",
                                value: "Delete",
                                "onclick": "deleteEventList('{gcal_id}', '{event_id}', '{calendar_name}')",
                            }
                        })
                    } else {
                        None
                    };
                    let start_time = get_default_or_local_time(event.start_time.into(), &config);
                    let end_time = get_default_or_local_time(event.end_time.into(), &config);
                    let name = &event.name;
                    let gcal_id = &event.gcal_id;
                    let event_id = &event.event_id;
    
                    rsx! {
                        tr {
                            key: "event-key-{idx}",
                            "text-style": "center",
                            td {
                                input {
                                    "type": "button",
                                    name: "{name}",
                                    value: "{name}",
                                    "onclick": "eventDetail('{gcal_id}', '{event_id}')",
                                }
                            },
                            td {"{start_time}"},
                            td {"{end_time}"},
                            td {delete},
                        }
                    }
                })
            }
        }
    })
}

pub fn event_detail_body(
    event: Event,
    config: Config,
) -> String {
    let mut app = VirtualDom::new_with_props(
        event_detail_element,
        event_detail_elementProps {
            event,
            config,
        },
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}

#[inline_props]
fn event_detail_element(
    cx: Scope,
    event: Event,
    config: Config,
) -> Element {
    let name = &event.name;
    let description = event.description.as_ref().map(|description| {
        let description = description
        .split('\n')
        .map(|line| {
            let mut line_length = 0;
            let words = line
                .split_whitespace()
                .map(|word| {
                    let mut output_word = StackString::new();
                    if let Ok(url) = word.parse::<Url>() {
                        if url.scheme() == "https" {
                            output_word = format_sstr!(r#"<a href="{url}">Link</a>"#);
                        }
                    } else {
                        output_word = word.into();
                    }
                    line_length += output_word.len();
                    if line_length > 60 {
                        output_word = format_sstr!("<br>{output_word}");
                        line_length = 0;
                    }
                    output_word
                })
                .join(" ");
            format_sstr!("\t\t{words}")
        })
        .join("");
        rsx! {"{description}"}
    });
    let start_time = get_default_or_local_time(event.start_time.into(), &config);
    let end_time = get_default_or_local_time(event.end_time.into(), &config);
    cx.render(rsx! {
        table {
            "border": "1",
            class: "dataframe",
            thead {
            },
            tbody {
                tr {
                    "text-style": "center",
                    td {"Name"},
                    td {"{name}"},
                },
                tr {
                    "text-style": "center",
                    td {"Description"},
                    td {description},
                },
                event.url.as_ref().map(|url| {
                    rsx! {
                        tr {
                            "text-style": "center",
                            td {"Url"},
                            td {
                                a {
                                    href: "{url}",
                                    "Link",
                                }
                            }
                        }
                    }
                }),
                event.location.as_ref().map(|location| {
                    let name = &location.name;
                    rsx! {
                        tr {
                            "text-style": "center",
                            td {"Location"},
                            td {"{name}"},
                        },
                        location.lat_lon.as_ref().map(|(lat, lon)| {
                            rsx! {
                                tr {
                                    "text-style": "center",
                                    td {"Lat,Lon"},
                                    td {"{lat},{lon}"},
                                }
                            }
                        }),
                    }
                }),
                tr {
                    "text-style": "center",
                    td {"Start Time"},
                    td {"{start_time}"},
                },
                tr {
                    "text-style": "center",
                    td {"End Time"},
                    td {"{end_time}"},
                }
            }
        }
    })
}

pub fn build_event_body(
    event: Event,
) -> String {
    let mut app = VirtualDom::new_with_props(
        build_calendar_event_element,
        build_calendar_event_elementProps {
            event,
        },
    );
    app.rebuild();
    dioxus::ssr::render_vdom(&app)
}

#[inline_props]
fn build_calendar_event_element(cx: Scope, event: Event) -> Element {
    let gcal_id = &event.gcal_id;
    let event_id = &event.event_id;
    let start_date = event.start_time.date();
    let start_time = event
        .start_time
        .time()
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_else(|_| "00:00".into());
    let end_date = event.end_time.date();
    let end_time = event
        .end_time
        .time()
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_else(|_| "00:00".into());
    let event_name = &event.name;
    let event_location_name = event.location.as_ref().map_or("", |l| l.name.as_str());
    let event_description = event.description.as_ref().map_or("", StackString::as_str);

    cx.render(rsx! {
        form {
            action: "javascript:createCalendarEvent();",
            table {
                "border": "1",
                tbody {
                    tr {
                        td {"Calendar ID:"},
                        td {
                            input {
                                "type": "text",
                                name: "gcal_id",
                                id: "gcal_id",
                                value: "{gcal_id}",
                            }
                        }
                    },
                    tr {
                        td {"Event ID:"},
                        td {
                            input {
                                "type": "text",
                                name: "event_id",
                                id: "event_id",
                                value: "{event_id}",
                            }
                        }
                    },
                    tr {
                        td {"Start Date:"},
                        td {
                            input {
                                "type": "date",
                                name: "start_date",
                                id: "start_date",
                                value: "{start_date}",
                            }
                        }
                    }
                    tr {
                        td {"Start Time:"},
                        td {
                            input {
                                "type": "time",
                                name: "start_time",
                                id: "start_time",
                                value: "{start_time}",
                            }
                        },
                    }
                    tr {
                        td {"End Date:"},
                        td {
                            input {
                                "type": "date",
                                name: "end_date",
                                id: "end_date",
                                value: "{end_date}",
                            }
                        }
                    },
                    tr {
                        td {"End Time:"},
                        td {
                            input {
                                "type": "time",
                                name: "end_time",
                                id: "end_time",
                                value: "{end_time}",
                            }
                        }
                    },
                    tr {
                        td {"Event Name:"},
                        td {
                            input {
                                "type": "text",
                                name: "event_name",
                                id: "event_name",
                                value: "{event_name}",
                            }
                        }
                    },
                    tr {
                        td {"Event Url:"},
                        td {
                            input {
                                "type": "url",
                                name: "event_url",
                                id: "event_url",
                                value: "https://localhost",
                            }
                        }
                    },
                    tr {
                        td {"Event Location Name:"},
                        td {
                            input {
                                "type": "text",
                                name: "event_location_name",
                                id: "event_location_name",
                                value: "{event_location_name}",
                            }
                        }
                    },
                    tr {
                        td {"Event Description:"},
                        td {
                            textarea {
                                cols: "40",
                                rows: "20",
                                name: "event_description",
                                id: "event_description",
                                "{event_description}",
                            }
                        }
                    },
                    tr {
                        td {
                            input {
                                "type": "button",
                                name: "create_event",
                                value: "Create Event",
                                "onclick": "createCalendarEvent();",
                            }
                        }
                    }
                }
            }
        }
    })
}