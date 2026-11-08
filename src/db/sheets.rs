//! Google Sheets backend (Apps Script Web App over HTTP).
//!
//! Ported from `App/database/sheet_db.py`. The Apps Script side
//! (`apps-script/gsheet_api.gs`) needs **no changes** — the contract is
//! HTTP-level, so existing deployments keep working.
//!
//! ## Error message parity
//!
//! The Python client has carefully worded messages for the failure modes that
//! actually happen when the Apps Script deployment is wrong. They are
//! reproduced here **verbatim**, because they are the difference between a user
//! fixing their deployment in one minute and giving up:
//!
//! * a redirect to the Google sign-in page,
//! * a non-JSON response (the script is not deployed as a public Web App),
//! * a 404 (wrong deployment URL),
//! * a 302 (the pre-`/exec` URL instead of the deployment URL),
//! * a transport failure.

use std::sync::RwLock;
use std::time::Duration;

use serde_json::{Map, Value};

use super::{normalize_table_name, Backend, DbError, Member, Row, ScanOutcome};

/// Default request timeout, matching the Python client's `timeout=5`.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 5;

/// Shown when the request was answered by the Google sign-in page.
pub const SIGN_IN_REDIRECT_MESSAGE: &str = "Redirected to Google sign-in. Deploy the script as a Web App with public access, then use the deployment URL (ends with /exec).";

/// Shown when the script answered with something that is not JSON.
pub const NOT_DEPLOYED_MESSAGE: &str = "The script is not deployed as a Web App or is not publicly accessible. Go to Extensions → Apps Script → Deploy → New Deployment → Web App (Execute as: Me, Access: Anyone).";

/// Shown on a 404.
pub const NOT_FOUND_MESSAGE: &str = "URL not found. Check the deployment URL.";

/// Shown on a 302.
pub const REDIRECT_MESSAGE: &str =
    "The URL redirects. Use the final deployment URL (ends with /exec).";

/// Shown when no deployment URL has been configured yet.
pub const NOT_CONFIGURED_MESSAGE: &str = "Google Sheets API URL not configured";

/// Configuration of the main spreadsheet, as reported by the `config` action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpreadsheetConfig {
    pub spreadsheet_id: Option<String>,
    pub configured: bool,
    pub spreadsheet_name: Option<String>,
    pub spreadsheet_url: Option<String>,
    pub error: Option<String>,
}

/// Registry entry of a single event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventDetails {
    pub event_name: String,
    pub spreadsheet_id: String,
    pub spreadsheet_url: String,
    pub created_date: String,
    pub status: String,
}

/// Which HTTP verb to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verb {
    Get,
    Post,
}

/// Google Sheets implementation of [`Backend`].
pub struct SheetBackend {
    client: reqwest::blocking::Client,
    api_url: RwLock<String>,
}

impl SheetBackend {
    /// A backend pointed at a deployment URL.
    ///
    /// The URL may be empty — the Python client also allows that and only
    /// fails when a request is actually made.
    pub fn new(api_url: &str) -> Result<Self, DbError> {
        SheetBackend::with_timeout(api_url, DEFAULT_TIMEOUT_SECONDS)
    }

    /// A backend with a custom request timeout, in seconds.
    pub fn with_timeout(api_url: &str, timeout_seconds: u64) -> Result<Self, DbError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| DbError::Backend(format!("Request failed (HTTP N/A): {e}")))?;
        Ok(SheetBackend {
            client,
            api_url: RwLock::new(api_url.to_string()),
        })
    }

    /// Point the backend at another deployment URL.
    pub fn set_api_url(&self, url: &str) {
        if let Ok(mut current) = self.api_url.write() {
            *current = url.to_string();
        }
    }

    /// The configured deployment URL.
    pub fn api_url(&self) -> String {
        self.api_url
            .read()
            .map(|url| url.clone())
            .unwrap_or_default()
    }

    /// `SheetDB.db_exists` — is a deployment URL configured?
    pub fn db_exists(&self) -> bool {
        !self.api_url().is_empty()
    }

    /// `SheetDB.get_config` — read the spreadsheet configuration.
    pub fn get_config(&self) -> Result<Option<SpreadsheetConfig>, DbError> {
        let result = self.request(Verb::Get, "config", &[], None)?;
        if !is_success(&result) {
            return Ok(None);
        }
        let Some(data) = result.get("data") else {
            return Ok(None);
        };
        Ok(Some(SpreadsheetConfig {
            spreadsheet_id: optional_string(data.get("spreadsheetId")),
            configured: data
                .get("configured")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            spreadsheet_name: optional_string(data.get("spreadsheetName")),
            spreadsheet_url: optional_string(data.get("spreadsheetUrl")),
            error: optional_string(data.get("error")),
        }))
    }

    /// `SheetDB.set_spreadsheet` — point the script at a spreadsheet.
    pub fn set_spreadsheet(&self, url_or_id: &str) -> Result<bool, DbError> {
        let result = self.request(Verb::Get, "setConfig", &[("url", url_or_id)], None)?;
        Ok(result
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// `SheetDB.get_event_details` — the registry entry of one event.
    pub fn get_event_details(&self, event_name: &str) -> Result<Option<EventDetails>, DbError> {
        let result = self.request(
            Verb::Post,
            "getEventDetails",
            &[],
            Some(json_object([("eventName", event_name)])),
        )?;
        if !is_success(&result) {
            return Ok(None);
        }
        let Some(data) = result.get("data") else {
            return Ok(None);
        };
        Ok(Some(EventDetails {
            event_name: string_or_default(data.get("eventName")),
            spreadsheet_id: string_or_default(data.get("spreadsheetId")),
            spreadsheet_url: string_or_default(data.get("spreadsheetUrl")),
            created_date: string_or_default(data.get("createdDate")),
            status: {
                let status = string_or_default(data.get("status"));
                if status.is_empty() {
                    "active".to_string()
                } else {
                    status
                }
            },
        }))
    }

    /// Perform one Apps Script call.
    ///
    /// `action` is sent as a query parameter and `payload` as a JSON body,
    /// exactly like the Python client.
    fn request(
        &self,
        verb: Verb,
        action: &str,
        params: &[(&str, &str)],
        payload: Option<Value>,
    ) -> Result<Value, DbError> {
        let api_url = self.api_url();
        if api_url.is_empty() {
            return Err(DbError::Backend(NOT_CONFIGURED_MESSAGE.to_string()));
        }

        let mut url = reqwest::Url::parse(&api_url)
            .map_err(|e| DbError::Backend(format!("Request failed (HTTP N/A): {e}")))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("action", action);
            for (key, value) in params {
                query.append_pair(key, value);
            }
        }

        let sent = match verb {
            Verb::Get => self.client.get(url.clone()).send(),
            Verb::Post => {
                let body = payload.unwrap_or_else(|| Value::Object(Map::new()));
                self.client.post(url.clone()).json(&body).send()
            }
        };

        let response = match sent {
            Ok(response) => response,
            Err(e) => return Err(DbError::Backend(format!("Request failed (HTTP N/A): {e}"))),
        };

        let status = response.status();
        let final_url = response.url().to_string();

        // A redirect to the Google sign-in page means the deployment is not
        // public, or the wrong URL was pasted.
        if final_url.contains("accounts.google.com") {
            return Err(DbError::Backend(SIGN_IN_REDIRECT_MESSAGE.to_string()));
        }

        // `raise_for_status` only raises for 4xx and 5xx; a 302 falls through
        // to the non-JSON branch, exactly like in the Python client.
        if status.is_client_error() || status.is_server_error() {
            let code = status.as_u16();
            let kind = if status.is_client_error() {
                "Client Error"
            } else {
                "Server Error"
            };
            let reason = status.canonical_reason().unwrap_or("");
            return Err(DbError::Backend(format!(
                "Request failed (HTTP {code}): {code} {kind}: {reason} for url: {final_url}"
            )));
        }

        let body = response.text().unwrap_or_default();
        match serde_json::from_str::<Value>(&body) {
            Ok(value) => Ok(value),
            Err(_) => Err(DbError::Backend(non_json_message(status.as_u16(), &body))),
        }
    }
}

/// Build the "API returned non-JSON" message.
///
/// Mirrors the Python client's `json.JSONDecodeError` branch, including the
/// 200 character preview and the order in which the cases are checked.
pub fn non_json_message(status: u16, body: &str) -> String {
    let preview = preview(body);
    let mut message = format!("API returned non-JSON (HTTP {status}). ");
    if preview.contains("accounts.google.com") || preview.to_lowercase().contains("signin") {
        message.push_str(NOT_DEPLOYED_MESSAGE);
    } else if status == 404 {
        message.push_str(NOT_FOUND_MESSAGE);
    } else if status == 302 {
        message.push_str(REDIRECT_MESSAGE);
    } else {
        message.push_str(&format!("Response preview: {preview}"));
    }
    message
}

/// The first 200 bytes of the body, or `(empty)`.
fn preview(body: &str) -> String {
    if body.is_empty() {
        return "(empty)".to_string();
    }
    let bytes = body.as_bytes();
    let end = bytes.len().min(200);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn is_success(result: &Value) -> bool {
    result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn string_or_default(value: Option<&Value>) -> String {
    optional_string(value).unwrap_or_default()
}

/// Build a JSON object from `(key, value)` pairs.
fn json_object<const N: usize>(pairs: [(&str, &str); N]) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
    Value::Object(map)
}

/// Turn a JSON array of objects into table rows.
fn rows_from_json(value: &Value, columns: &[&str]) -> Vec<Row> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let cells = columns
                .iter()
                .map(|column| {
                    let text = match object.get(*column) {
                        Some(Value::String(text)) => text.clone(),
                        Some(Value::Number(number)) => number.to_string(),
                        Some(Value::Bool(flag)) => flag.to_string(),
                        _ => String::new(),
                    };
                    ((*column).to_string(), text)
                })
                .collect();
            Some(Row::new(cells))
        })
        .collect()
}

/// The columns the `Members` table shows in the UI.
const MEMBER_COLUMNS: &[&str] = &["rfid", "memberid", "name", "student_num", "date_registered"];

/// The columns an event table shows in the UI.
const ATTENDANCE_COLUMNS: &[&str] = &["memberid", "name", "student_num", "attendance_time"];

impl Backend for SheetBackend {
    fn member_register(
        &self,
        rfid: &str,
        memberid: &str,
        name: &str,
        student_num: &str,
        program: &str,
        year: &str,
    ) -> Result<(), DbError> {
        let date_registered = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut payload = Map::new();
        payload.insert("rfid".into(), Value::String(rfid.into()));
        payload.insert("memberid".into(), Value::String(memberid.into()));
        payload.insert("name".into(), Value::String(name.into()));
        payload.insert("student_num".into(), Value::String(student_num.into()));
        payload.insert("program".into(), Value::String(program.into()));
        payload.insert("year".into(), Value::String(year.into()));
        payload.insert(
            "date_registered".into(),
            Value::String(date_registered.clone()),
        );

        let result = self.request(Verb::Post, "addMember", &[], Some(Value::Object(payload)))?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }

    fn member_exists(&self, rfid: &str) -> Result<Option<Member>, DbError> {
        let result = self.request(
            Verb::Post,
            "getMember",
            &[],
            Some(json_object([("rfid", rfid)])),
        )?;
        if !is_success(&result) {
            return Ok(None);
        }
        let Some(data) = result.get("data") else {
            return Ok(None);
        };
        if data.is_null() {
            return Ok(None);
        }
        Ok(Some(Member {
            memberid: string_or_default(data.get("memberid")),
            name: string_or_default(data.get("name")),
            student_num: string_or_default(data.get("student_num")),
        }))
    }

    fn list_tables(&self) -> Result<Vec<String>, DbError> {
        let result = self.request(Verb::Get, "listEvents", &[], None)?;
        if !is_success(&result) {
            return Ok(Vec::new());
        }
        let Some(items) = result.get("data").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(items
            .iter()
            .map(|item| match item {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .collect())
    }

    fn create_event_table(&self, table_name: &str) -> Result<(), DbError> {
        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "createEvent",
            &[],
            Some(json_object([("eventName", &event_name)])),
        )?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }

    fn fetch_table_data(&self, table_name: &str) -> Result<Vec<Row>, DbError> {
        if table_name == "Members" {
            let result = self.request(Verb::Get, "listMembers", &[], None)?;
            if !is_success(&result) {
                return Ok(Vec::new());
            }
            let Some(data) = result.get("data") else {
                return Ok(Vec::new());
            };
            return Ok(rows_from_json(data, MEMBER_COLUMNS));
        }

        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "getAttendees",
            &[],
            Some(json_object([("eventName", &event_name)])),
        )?;
        if !is_success(&result) {
            return Ok(Vec::new());
        }
        let Some(data) = result.get("data") else {
            return Ok(Vec::new());
        };
        Ok(rows_from_json(data, ATTENDANCE_COLUMNS))
    }

    fn attendance_member_event(&self, table_name: &str, rfid: &str) -> Result<(), DbError> {
        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "recordAttendance",
            &[],
            Some(json_object([("eventName", &event_name), ("rfid", rfid)])),
        )?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }

    fn scan_attendance(&self, table_name: &str, rfid: &str) -> Result<ScanOutcome, DbError> {
        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "scanAttendance",
            &[],
            Some(json_object([("eventName", &event_name), ("rfid", rfid)])),
        )?;
        Ok(ScanOutcome {
            success: is_success(&result),
            attended: result
                .get("attended")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            error: result
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    fn get_member_name(&self, rfid: &str) -> Result<Option<String>, DbError> {
        Ok(self.member_exists(rfid)?.map(|member| member.name))
    }

    fn member_attended_event(&self, table_name: &str, rfid: &str) -> Result<bool, DbError> {
        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "checkAttended",
            &[],
            Some(json_object([("eventName", &event_name), ("rfid", rfid)])),
        )?;
        if !is_success(&result) {
            return Ok(false);
        }
        Ok(result
            .get("attended")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    fn add_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        let mut payload = Map::new();
        payload.insert("rfid".into(), Value::String(rfid.into()));
        payload.insert("points".into(), Value::from(points));
        let result = self.request(Verb::Post, "addPoints", &[], Some(Value::Object(payload)))?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }

    fn get_member_points(&self, rfid: &str) -> Result<f64, DbError> {
        let result = self.request(
            Verb::Post,
            "getMember",
            &[],
            Some(json_object([("rfid", rfid)])),
        )?;
        if !is_success(&result) {
            return Ok(0.0);
        }
        let Some(data) = result.get("data") else {
            return Ok(0.0);
        };
        Ok(data.get("points").and_then(Value::as_f64).unwrap_or(0.0))
    }

    fn redeem_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        let mut payload = Map::new();
        payload.insert("rfid".into(), Value::String(rfid.into()));
        payload.insert("points".into(), Value::from(points));
        let result = self.request(
            Verb::Post,
            "redeemPoints",
            &[],
            Some(Value::Object(payload)),
        )?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }

    fn delete_event_table(&self, table_name: &str) -> Result<(), DbError> {
        let event_name = normalize_table_name(table_name)?;
        let result = self.request(
            Verb::Post,
            "deleteEvent",
            &[],
            Some(json_object([("eventName", &event_name)])),
        )?;
        if is_success(&result) {
            Ok(())
        } else {
            Err(DbError::Backend(error_message(&result)))
        }
    }
}

/// The `error` field of an Apps Script response, or a generic fallback.
fn error_message(result: &Value) -> String {
    result
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("Unknown error")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_is_capped_at_two_hundred_bytes() {
        let body = "x".repeat(500);
        let message = non_json_message(200, &body);
        assert!(message.contains(&"x".repeat(200)));
        assert!(!message.contains(&"x".repeat(201)));
    }

    #[test]
    fn an_empty_body_previews_as_empty() {
        let message = non_json_message(200, "");
        assert!(message.contains("Response preview: (empty)"));
    }

    #[test]
    fn a_sign_in_page_is_reported_as_not_deployed() {
        let message = non_json_message(200, "<html>accounts.google.com/signin</html>");
        assert_eq!(
            message,
            format!("API returned non-JSON (HTTP 200). {NOT_DEPLOYED_MESSAGE}")
        );
    }

    #[test]
    fn a_404_is_reported_as_a_missing_url() {
        assert_eq!(
            non_json_message(404, "<html>nope</html>"),
            format!("API returned non-JSON (HTTP 404). {NOT_FOUND_MESSAGE}")
        );
    }

    #[test]
    fn a_302_is_reported_as_a_redirect() {
        assert_eq!(
            non_json_message(302, "<html>redirect</html>"),
            format!("API returned non-JSON (HTTP 302). {REDIRECT_MESSAGE}")
        );
    }

    #[test]
    fn any_other_failure_shows_the_preview() {
        let message = non_json_message(500, "<html>boom</html>");
        assert_eq!(
            message,
            "API returned non-JSON (HTTP 500). Response preview: <html>boom</html>"
        );
    }
}
