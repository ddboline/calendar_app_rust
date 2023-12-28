!function() {
    displayAgenda();
}()
function displayAgenda() {
    let url = "/calendar/agenda";
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = "&nbsp;";
        document.getElementById("main_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("GET", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function syncCalendars() {
    let url = "/calendar/sync_calendars";
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("POST", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function syncCalendarsFull() {
    let url = "/calendar/sync_calendars_full";
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("POST", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function deleteEvent(gcal_id, event_id, callback=null) {
    let url = "/calendar/delete_event";
    let data = JSON.stringify({'gcal_id': gcal_id, 'event_id': event_id});
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
        if (callback) {
            callback();
        }
    }
    xmlhttp.open("DELETE", url, true);
    xmlhttp.setRequestHeader('Content-Type', 'application/json');
    xmlhttp.send(data);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function deleteEventAgenda(gcal_id, event_id) {
    deleteEvent(gcal_id, event_id, () => displayAgenda());
}
function deleteEventList(gcal_id, event_id, calendar_name) {
    deleteEvent(gcal_id, event_id, () => listEvents(calendar_name));
}
function eventDetail(gcal_id, event_id) {
    let url = "/calendar/event_detail";
    let data = JSON.stringify({'gcal_id': gcal_id, 'event_id': event_id});
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("POST", url, true);
    xmlhttp.setRequestHeader('Content-Type', 'application/json');
    xmlhttp.send(data);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function listCalendars() {
    let url = "/calendar/list_calendars";
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("main_article").innerHTML = xmlhttp.responseText;
        document.getElementById("sub_article").innerHTML = "&nbsp;";
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("GET", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function listEvents(calendar_name) {
    let url = `/calendar/list_events?calendar_name=${calendar_name}`;
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("main_article").innerHTML = xmlhttp.responseText;
        document.getElementById("sub_article").innerHTML = "&nbsp;";
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("GET", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function buildEvent(gcal_id, event_id=null) {
    let url = `/calendar/create_calendar_event?gcal_id=${gcal_id}`;
    if (event_id) {
        url = `${url}&event_id=${event_id}`;
    }
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function f() {
        document.getElementById("sub_article").innerHTML = xmlhttp.responseText;
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.open("GET", url, true);
    xmlhttp.send(null);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function createCalendarEvent() {
    let url = "/calendar/create_calendar_event";

    let gcal_id = document.getElementById("gcal_id").value;
    let event_id = document.getElementById("event_id").value;
    let event_start_date = document.getElementById("start_date").value;
    let event_start_time = document.getElementById("start_time").value;
    let event_end_date = document.getElementById("end_date").value;
    let event_end_time = document.getElementById("end_time").value;
    let event_url = document.getElementById("event_url").value;
    let event_name = document.getElementById("event_name").value;
    let event_description = document.getElementById("event_description").value;
    let event_location_name = document.getElementById("event_location_name").value;

    let data = JSON.stringify({
        "gcal_id": gcal_id,
        "event_id": event_id,
        "event_start_datetime": `${event_start_date}T${event_start_time}:00`,
        "event_end_datetime": `${event_start_date}T${event_end_time}:00`,
        "event_url": event_url,
        "event_name": event_name,
        "event_description": event_description,
        "event_location_name": event_location_name,
    });

    let xmlhttp = new XMLHttpRequest();
    xmlhttp.open('POST', url, true);
    xmlhttp.onload = function see_result() {
        document.getElementById("sub_article").innerHTML = "&nbsp;";
        document.getElementById("garminconnectoutput").innerHTML = "done";
    }
    xmlhttp.setRequestHeader('Content-Type', 'application/json');
    xmlhttp.send(data);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
function calendarDisplay(gcal_id, display) {
    let url = `/calendar/edit_calendar/${gcal_id}`
    let data = jSON.stringify({"display": display});
    let xmlhttp = new XMLHttpRequest();
    xmlhttp.onload = function see_result() {
        listCalendars();
    }
    xmlhttp.open("POST", url, true);
    xmlhttp.send(data);
    document.getElementById("garminconnectoutput").innerHTML = "running";
}
