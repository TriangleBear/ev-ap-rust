//! A modal form.
//!
//! The Python app opens `CTk.CTkToplevel` windows for the member
//! registration form, the event name form and the report selection. Those
//! become modals here.

use egui::Context;

/// A field of a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormField {
    /// A single line text field.
    Text {
        /// The label above the field.
        label: String,
        /// The greyed out hint shown while the field is empty.
        placeholder: String,
        /// The current value.
        value: String,
    },
    /// A drop down with a fixed set of options.
    Choice {
        /// The label above the field.
        label: String,
        /// The available options.
        options: Vec<String>,
        /// The index of the selected option.
        selected: usize,
    },
}

impl FormField {
    /// A text field.
    pub fn text(label: &str, placeholder: &str) -> Self {
        FormField::Text {
            label: label.to_string(),
            placeholder: placeholder.to_string(),
            value: String::new(),
        }
    }

    /// A drop down. The first option is selected.
    pub fn choice(label: &str, options: Vec<String>) -> Self {
        FormField::Choice {
            label: label.to_string(),
            options,
            selected: 0,
        }
    }

    /// The value of the field.
    pub fn value(&self) -> String {
        match self {
            FormField::Text { value, .. } => value.clone(),
            FormField::Choice {
                options, selected, ..
            } => options.get(*selected).cloned().unwrap_or_default(),
        }
    }
}

/// What the user did with a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormOutcome {
    /// The form is still open.
    Idle,
    /// The submit button was pressed.
    Submitted,
    /// The form was cancelled.
    Cancelled,
}

/// A modal form with a title, a set of fields and a submit button.
pub struct FormModal {
    /// The dialog title.
    pub title: String,
    /// The fields.
    pub fields: Vec<FormField>,
    /// The label of the submit button.
    pub submit_label: String,
    /// A message shown above the fields.
    pub error: Option<String>,
}

impl FormModal {
    /// A form with the given title and fields.
    pub fn new(title: &str, fields: Vec<FormField>) -> Self {
        FormModal {
            title: title.to_string(),
            fields,
            submit_label: "Submit".to_string(),
            error: None,
        }
    }

    /// Change the label of the submit button.
    pub fn with_submit_label(mut self, label: &str) -> Self {
        self.submit_label = label.to_string();
        self
    }

    /// Show a message above the fields.
    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }

    /// The values of the fields, in order.
    pub fn values(&self) -> Vec<String> {
        self.fields.iter().map(FormField::value).collect()
    }

    /// The value of one field.
    pub fn value(&self, index: usize) -> String {
        self.fields
            .get(index)
            .map(FormField::value)
            .unwrap_or_default()
    }

    /// Draw the modal and report what the user did.
    pub fn show(&mut self, ctx: &Context) -> FormOutcome {
        let mut outcome = FormOutcome::Idle;

        egui::Modal::new(egui::Id::new(&self.title))
            .frame(egui::Frame::popup(&ctx.style()).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.heading(&self.title);
                ui.separator();

                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                    ui.add_space(6.0);
                }

                for field in &mut self.fields {
                    match field {
                        FormField::Text {
                            label,
                            placeholder,
                            value,
                        } => {
                            ui.label(label.as_str());
                            ui.add(
                                egui::TextEdit::singleline(value)
                                    .hint_text(placeholder.as_str())
                                    .desired_width(f32::INFINITY),
                            );
                            ui.add_space(4.0);
                        }
                        FormField::Choice {
                            label,
                            options,
                            selected,
                        } => {
                            ui.label(label.as_str());
                            let selected_text = options
                                .get(*selected)
                                .cloned()
                                .unwrap_or_else(|| "—".to_string());
                            egui::ComboBox::from_id_salt(label)
                                .selected_text(selected_text)
                                .width(f32::INFINITY)
                                .show_ui(ui, |ui| {
                                    for (index, option) in options.iter().enumerate() {
                                        ui.selectable_value(selected, index, option);
                                    }
                                });
                            ui.add_space(4.0);
                        }
                    }
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(&self.submit_label).clicked() {
                        outcome = FormOutcome::Submitted;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = FormOutcome::Cancelled;
                    }
                });
            });

        outcome
    }
}
