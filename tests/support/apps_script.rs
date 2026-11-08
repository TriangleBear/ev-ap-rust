//! A stateful fake of the Google Apps Script Web App.
//!
//! The route based fake in `mod.rs` answers with canned responses. This one
//! actually keeps the members and the attendance, so the *same* scenario can
//! be run against the SQLite backend and against the Sheets backend and both
//! must produce the same result — which is what the parity check of Phase 7
//! needs.
//!
//! The behaviour follows `apps-script/gsheet_api.gs`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::{json, Value};

/// A member in the fake `Members` sheet.
#[derive(Debug, Clone)]
pub struct Member {
    pub rfid: String,
    pub memberid: String,
    pub name: String,
    pub student_num: String,
    pub program: String,
    pub year: String,
    pub date_registered: String,
}

impl Member {
    fn to_json(&self) -> Value {
        json!({
            "rfid": self.rfid,
            "memberid": self.memberid,
            "name": self.name,
            "student_num": self.student_num,
            "program": self.program,
            "year": self.year,
            "date_registered": self.date_registered,
        })
    }
}

/// One attendance row.
#[derive(Debug, Clone)]
pub struct Attendance {
    pub rfid: String,
    pub memberid: String,
    pub student_num: String,
    pub name: String,
    pub attendance_time: String,
}

impl Attendance {
    fn to_json(&self) -> Value {
        json!({
            "rfid": self.rfid,
            "memberid": self.memberid,
            "student_num": self.student_num,
            "name": self.name,
            "attendance_time": self.attendance_time,
        })
    }
}

/// The state of the fake script.
#[derive(Debug, Default)]
pub struct FakeState {
    pub members: Vec<Member>,
    /// event name -> attendance rows
    pub events: BTreeMap<String, Vec<Attendance>>,
    pub spreadsheet_id: String,
    pub spreadsheet_name: String,
}

impl FakeState {
    pub fn member(&self, rfid: &str) -> Option<&Member> {
        self.members.iter().find(|member| member.rfid == rfid)
    }

    pub fn event_names(&self) -> Vec<String> {
        self.events.keys().cloned().collect()
    }

    /// Handle one Apps Script action.
    pub fn handle(&mut self, action: &str, payload: &Value) -> Value {
        match action {
            "addMember" => {
                let rfid = string(payload, "rfid");
                if self.member(&rfid).is_some() {
                    return json!({"success": false, "error": "Member already exists"});
                }
                self.members.push(Member {
                    rfid,
                    memberid: string(payload, "memberid"),
                    name: string(payload, "name"),
                    student_num: string(payload, "student_num"),
                    program: string(payload, "program"),
                    year: string(payload, "year"),
                    date_registered: string(payload, "date_registered"),
                });
                json!({"success": true})
            }
            "getMember" => {
                let rfid = string(payload, "rfid");
                match self.member(&rfid) {
                    Some(member) => json!({"success": true, "data": member.to_json()}),
                    None => json!({"success": true, "data": Value::Null}),
                }
            }
            "listMembers" => {
                let members: Vec<Value> = self.members.iter().map(Member::to_json).collect();
                json!({"success": true, "data": members})
            }
            "listEvents" => json!({"success": true, "data": self.event_names()}),
            "createEvent" => {
                let name = string(payload, "eventName").replace(' ', "_");
                self.events.entry(name.clone()).or_default();
                json!({"success": true, "eventName": name, "spreadsheetId": format!("sheet-{name}"), "spreadsheetUrl": format!("https://docs.google.com/spreadsheets/d/sheet-{name}")})
            }
            "deleteEvent" => {
                let name = string(payload, "eventName");
                self.events.remove(&name);
                json!({"success": true})
            }
            "getAttendees" => {
                let name = string(payload, "eventName");
                let rows: Vec<Value> = self
                    .events
                    .get(&name)
                    .map(|rows| rows.iter().map(Attendance::to_json).collect())
                    .unwrap_or_default();
                json!({"success": true, "data": rows})
            }
            "recordAttendance" => {
                let name = string(payload, "eventName");
                let rfid = string(payload, "rfid");
                let Some(member) = self.member(&rfid).cloned() else {
                    return json!({"success": false, "error": "No member found with the given RFID"});
                };
                if !self.events.contains_key(&name) {
                    return json!({"success": false, "error": format!("Event not found: {name}")});
                }
                let row = Attendance {
                    rfid: member.rfid.clone(),
                    memberid: member.memberid.clone(),
                    student_num: member.student_num.clone(),
                    name: member.name.clone(),
                    attendance_time: "2026-01-01 10:00:00".to_string(),
                };
                self.events.get_mut(&name).unwrap().push(row);
                json!({"success": true})
            }
            "checkAttended" => {
                let name = string(payload, "eventName");
                let rfid = string(payload, "rfid");
                let attended = self
                    .events
                    .get(&name)
                    .map(|rows| rows.iter().any(|row| row.rfid == rfid))
                    .unwrap_or(false);
                json!({"success": true, "attended": attended})
            }
            "scanAttendance" => {
                let name = string(payload, "eventName");
                let rfid = string(payload, "rfid");
                let Some(member) = self.member(&rfid).cloned() else {
                    return json!({"success": false, "error": "RFID number not found"});
                };
                let attended = self
                    .events
                    .get(&name)
                    .map(|rows| rows.iter().any(|row| row.rfid == rfid))
                    .unwrap_or(false);
                if attended {
                    return json!({"success": true, "attended": true, "member": member.to_json()});
                }
                if !self.events.contains_key(&name) {
                    return json!({"success": false, "error": format!("Event not found: {name}")});
                }
                let row = Attendance {
                    rfid: member.rfid.clone(),
                    memberid: member.memberid.clone(),
                    student_num: member.student_num.clone(),
                    name: member.name.clone(),
                    attendance_time: "2026-01-01 10:00:00".to_string(),
                };
                self.events.get_mut(&name).unwrap().push(row);
                json!({"success": true, "attended": false, "member": member.to_json(), "attendanceTime": "2026-01-01 10:00:00"})
            }
            "addPoints" | "redeemPoints" => {
                json!({"success": false, "error": format!("Unknown action: {action}")})
            }
            "config" => json!({
                "success": true,
                "data": {
                    "spreadsheetId": self.spreadsheet_id,
                    "configured": !self.spreadsheet_id.is_empty(),
                    "spreadsheetName": self.spreadsheet_name,
                    "spreadsheetUrl": format!("https://docs.google.com/spreadsheets/d/{}", self.spreadsheet_id),
                }
            }),
            "setConfig" => json!({"success": true, "data": {"spreadsheetId": "configured"}}),
            other => json!({"success": false, "error": format!("Unknown action: {other}")}),
        }
    }
}

/// Read a string from a payload.
fn string(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A handle to the fake script's state, for assertions.
pub type SharedState = Arc<Mutex<FakeState>>;

/// The shared state type.
pub use std::sync::Arc;

/// Create the shared state of a fake script.
pub fn new_state() -> SharedState {
    Arc::new(Mutex::new(FakeState {
        spreadsheet_id: "sheet-main".to_string(),
        spreadsheet_name: "Ev_Ap Main".to_string(),
        ..Default::default()
    }))
}
