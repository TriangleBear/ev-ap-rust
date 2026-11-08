//! The modal message box.
//!
//! Replaces `CTkMessagebox` from the Python app. The Python message box is a
//! modal window with one or two buttons; the egui equivalent is an
//! [`egui::Modal`], which dims the background and blocks input to the rest of
//! the application while it is open.

use egui::Context;

/// The answer the user gave in a message box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxResponse {
    /// The single "OK" button was pressed.
    Ok,
    /// The first button of a confirmation was pressed.
    Yes,
    /// The second button of a confirmation was pressed.
    No,
}

/// A modal dialog with a title, a message and one or two buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageBox {
    /// The window title.
    pub title: String,
    /// The message body.
    pub message: String,
    /// The buttons, in the order they are shown.
    pub buttons: Vec<(String, MessageBoxResponse)>,
    /// The answer, set once the user pressed a button.
    pub response: Option<MessageBoxResponse>,
}

impl MessageBox {
    /// A message box with a single "OK" button.
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        MessageBox {
            title: title.into(),
            message: message.into(),
            buttons: vec![("OK".to_string(), MessageBoxResponse::Ok)],
            response: None,
        }
    }

    /// A message box with a "Yes" and a "No" button.
    pub fn confirm(
        title: impl Into<String>,
        message: impl Into<String>,
        yes_label: &str,
        no_label: &str,
    ) -> Self {
        MessageBox {
            title: title.into(),
            message: message.into(),
            buttons: vec![
                (yes_label.to_string(), MessageBoxResponse::Yes),
                (no_label.to_string(), MessageBoxResponse::No),
            ],
            response: None,
        }
    }
}

/// Draw the message box, if there is one.
///
/// The answer is written into the box; the caller reads it afterwards and
/// closes the dialog.
pub fn show(ctx: &Context, dialog: &mut Option<MessageBox>) {
    let Some(dialog) = dialog.as_mut() else {
        return;
    };
    if dialog.response.is_some() {
        return;
    }

    let mut clicked: Option<MessageBoxResponse> = None;

    egui::Modal::new(egui::Id::new("ev_ap_message_box"))
        .frame(egui::Frame::popup(&ctx.style()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.set_max_width(480.0);

            ui.heading(&dialog.title);
            ui.separator();
            ui.label(&dialog.message);
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                for (label, response) in &dialog.buttons {
                    if ui.button(label).clicked() {
                        clicked = Some(*response);
                    }
                }
            });
        });

    if let Some(response) = clicked {
        dialog.response = Some(response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_info_box_has_a_single_ok_button() {
        let dialog = MessageBox::info("RFID Scan", "RFID recorded");
        assert_eq!(dialog.title, "RFID Scan");
        assert_eq!(dialog.message, "RFID recorded");
        assert_eq!(dialog.buttons.len(), 1);
        assert_eq!(dialog.buttons[0].1, MessageBoxResponse::Ok);
        assert!(dialog.response.is_none());
    }

    #[test]
    fn a_confirm_box_has_a_yes_and_a_no_button() {
        let dialog = MessageBox::confirm("Create Event", "Are you sure?", "Yes", "No");
        assert_eq!(dialog.buttons.len(), 2);
        assert_eq!(dialog.buttons[0].1, MessageBoxResponse::Yes);
        assert_eq!(dialog.buttons[1].1, MessageBoxResponse::No);
    }
}
