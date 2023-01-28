use crate::{
    data::{escalator_input::EscalatorInput, status::Status},
    generate::REPORT_EMOJI,
    prelude::*,
};

use super::Report;
use std::{ops::BitOr, str::FromStr};

const SELECTED_BUTTON_STYLE: serenity::ButtonStyle = serenity::ButtonStyle::Primary;
const UNSELECTED_BUTTON_STYLE: serenity::ButtonStyle = serenity::ButtonStyle::Secondary;

const PAIR_BUTTON_ID: &str = "REPORT-PAIR";
const ALL_BUTTON_ID: &str = "REPORT-ALL";
const SUBMIT_BUTTON_ID: &str = "REPORT-SUBMIT";

const NUMBER_BUTTON_ID_PREFIX: &str = "REPORT-FLOOR-";
const STATUS_BUTTON_ID_PREFIX: &str = "REPORT-STATUS-";

#[derive(Debug, Clone, Copy)]
pub struct ReportComponent {
    escalators: EscalatorComponent,
    status: Option<Status>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscalatorComponent {
    Floors {
        floors: Option<(u8, Option<u8>)>,
        pair: bool,
    },
    All,
}

pub enum ComponentStatus<T> {
    Continue,
    Complete(T),
}

pub enum ComponentAction {
    Escalator(EscalatorAction),
    Status(Status),
    Submit,
}

pub enum EscalatorAction {
    Pair,
    All,
    Floor(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Selected,
    Unselected,
    Disabled,
}

impl ReportComponent {
    pub fn new() -> Self {
        Self {
            escalators: EscalatorComponent::new(),
            status: None,
        }
    }

    pub fn render(&self) -> serenity::CreateComponents {
        let mut components = serenity::CreateComponents::default();

        // add escalator components
        components.0.append(&mut self.escalators.render().0);

        // selecting status
        components
            .create_action_row(|action_row| {
                for status in [Status::Open, Status::Down, Status::Blocked] {
                    let id = format!("{}{}", STATUS_BUTTON_ID_PREFIX, status.as_id_str());
                    let mut button = ButtonState::selected_if(Some(status) == self.status)
                        .or_else(|| ButtonState::disabled_if(self.status.is_some()))
                        .create_button("", id);

                    button.emoji(status.emoji());

                    action_row.add_button(button);
                }

                action_row
            })
            .create_action_row(|action_row| action_row.add_button(self.create_submit_button()));

        components
    }

    pub fn execute(&mut self, command: ComponentAction) -> ComponentStatus<Report> {
        match command {
            ComponentAction::Escalator(command) => {
                self.escalators.execute(command);
                ComponentStatus::Continue
            }
            ComponentAction::Status(status) => {
                match self.status {
                    Some(old_status) if old_status == status => {
                        self.status = None;
                    }
                    Some(_) => (),
                    None => self.status = Some(status),
                }

                ComponentStatus::Continue
            }
            ComponentAction::Submit => match self.try_as_report() {
                Some(report) => ComponentStatus::Complete(report),
                None => ComponentStatus::Continue,
            },
        }
    }

    fn try_as_report(&self) -> Option<Report> {
        let escalators = self.escalators.try_as_escalators()?;
        let status = self.status?;

        Some(Report { escalators, status })
    }

    fn create_submit_button(&self) -> serenity::CreateButton {
        let Some(escalators) = self.escalators.try_as_escalators() else {
            return Self::disabled_submit_button("Select Escalator(s)");
        };

        if self.status.is_none() {
            return Self::disabled_submit_button("Select Status");
        }

        let label = format!("Report {}!", escalators.short_noun());

        let mut button = serenity::CreateButton::default();
        button
            .custom_id(SUBMIT_BUTTON_ID)
            .style(serenity::ButtonStyle::Success)
            .emoji(REPORT_EMOJI)
            .label(label);

        button
    }

    fn disabled_submit_button(label: &str) -> serenity::CreateButton {
        let mut button = serenity::CreateButton::default();
        button
            .custom_id(SUBMIT_BUTTON_ID)
            .disabled(true)
            .style(serenity::ButtonStyle::Danger)
            .label(label);

        button
    }
}

impl EscalatorComponent {
    fn new() -> Self {
        Self::Floors {
            floors: None,
            pair: false,
        }
    }

    fn try_as_escalators(&self) -> Option<EscalatorInput> {
        match self {
            &Self::Floors {
                floors: Some((start, Some(end))),
                pair,
            } => {
                if pair {
                    Some(EscalatorInput::Pair(start, end))
                } else {
                    Some(EscalatorInput::Direct(start, end))
                }
            }
            Self::All => Some(EscalatorInput::All),
            _ => None,
        }
    }

    fn render(&self) -> serenity::CreateComponents {
        const NUMBER_PANEL: [[u8; 4]; 2] = [[2, 4, 6, 8], [3, 5, 7, 9]];

        let mut components = serenity::CreateComponents::default();

        for (row, numbers) in NUMBER_PANEL.into_iter().enumerate() {
            components.create_action_row(|action_row| {
                for floor in numbers {
                    action_row.add_button(self.create_floor_button(floor));
                }

                let button = match row {
                    // top row
                    0 => self.create_pair_button(),
                    // bottom row
                    1 => self.create_all_button(),
                    _ => unreachable!(),
                };

                action_row.add_button(button)
            });
        }

        components
    }

    fn execute(&mut self, command: EscalatorAction) {
        match command {
            EscalatorAction::Pair => {
                if let Self::Floors { pair, .. } = self {
                    *pair = !*pair
                }
            }
            EscalatorAction::All => match self {
                Self::Floors { .. } => *self = Self::All,
                Self::All => *self = Self::new(),
            },
            EscalatorAction::Floor(floor) => self.toggle_floor(floor),
        }
    }

    fn create_pair_button(&self) -> serenity::CreateButton {
        ButtonState::disabled_if(self.is_all())
            .or_else(|| ButtonState::selected_if(self.is_pair()))
            .create_button("Pair", PAIR_BUTTON_ID)
    }

    fn create_all_button(&self) -> serenity::CreateButton {
        ButtonState::selected_if(self.is_all()).create_button("All", ALL_BUTTON_ID)
    }

    fn create_floor_button(&self, floor: u8) -> serenity::CreateButton {
        let id = format!("{}{}", NUMBER_BUTTON_ID_PREFIX, floor);

        ButtonState::selected_if(self.is_floor_selected(floor))
            .or_else(|| ButtonState::disabled_if(!self.is_valid_next_floor(floor)))
            .create_button(floor, id)
    }

    fn is_all(&self) -> bool {
        match self {
            Self::Floors { .. } => false,
            Self::All => true,
        }
    }

    fn is_pair(&self) -> bool {
        match self {
            Self::Floors { pair, .. } => *pair,
            Self::All => false,
        }
    }

    fn toggle_floor(&mut self, floor: u8) {
        match self {
            Self::Floors { floors, .. } => {
                // if no floors are selected, set the start to the selected floor
                let Some((start, maybe_end)) = floors else {
                    *floors = Some((floor, None));
                    return;
                };

                // if the start is the selected floor
                if *start == floor {
                    // if there is an end selected, leave only the end selected
                    // and replace the start with the end
                    // otherwise, unselect all floors
                    if let Some(end) = maybe_end {
                        *floors = Some((*end, None));
                    } else {
                        *floors = None;
                    }
                    return;
                }

                // if the end is not selected
                let Some(end) = maybe_end else {
                    // if the start and selected floor create a valid escalator,
                    // set the end to the selected floor
                    if is_valid_escalator(*start, floor) {
                        *maybe_end = Some(floor);
                    }
                    return;
                };

                // if the end is the selected floor, unselect the end
                if *end == floor {
                    *maybe_end = None;
                }
            }
            Self::All => (),
        }
    }

    fn is_floor_selected(&self, floor: u8) -> bool {
        match self {
            &Self::Floors {
                floors: Some((start, maybe_end)),
                ..
            } => start == floor || maybe_end == Some(floor),
            _ => false,
        }
    }

    fn is_valid_next_floor(&self, floor: u8) -> bool {
        match self {
            Self::Floors { floors, .. } => match floors {
                Some((start, None)) => is_valid_escalator(*start, floor),
                Some((_, Some(_))) => false,
                None => true,
            },
            Self::All => false,
        }
    }
}

/// Checks if a pair of floors makes a valid escalator.
/// Currently, this doesn't try to make sure it is accurate
/// to the "escalators" table, so if that was to ever change,
/// this would need to be updated.
fn is_valid_escalator(start: u8, end: u8) -> bool {
    // if the start and end are valid floors...
    ((2..=9).contains(&start) && (2..=9).contains(&end))
        // ...and the distance between the two floors is 2
        //                           ...or it is the 2-3 or 3-2
        && (start.abs_diff(end) == 2 || matches!((start, end), (2, 3) | (3, 2)))
}

impl FromStr for ComponentAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(action) = s.parse::<EscalatorAction>() {
            return Ok(Self::Escalator(action));
        }

        if s == SUBMIT_BUTTON_ID {
            return Ok(Self::Submit);
        }

        if let Some(status) = s.strip_prefix(STATUS_BUTTON_ID_PREFIX) {
            return Ok(Self::Status(status.parse::<Status>()?));
        }

        anyhow::bail!("Unknown ReportAction format");
    }
}

impl FromStr for EscalatorAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == PAIR_BUTTON_ID {
            return Ok(Self::Pair);
        }

        if s == ALL_BUTTON_ID {
            return Ok(Self::All);
        }

        if let Some(floor) = s.strip_prefix(NUMBER_BUTTON_ID_PREFIX) {
            return Ok(Self::Floor(floor.parse::<u8>()?));
        }

        anyhow::bail!("Unknown EscalatorAction format");
    }
}

impl ButtonState {
    fn create_button(self, label: impl ToString, id: impl ToString) -> serenity::CreateButton {
        let mut button = serenity::CreateButton::default();

        let style = if self == Self::Selected {
            SELECTED_BUTTON_STYLE
        } else {
            UNSELECTED_BUTTON_STYLE
        };

        let disabled = self == Self::Disabled;

        button
            .style(style)
            .disabled(disabled)
            .custom_id(id)
            .label(label);

        button
    }

    fn disabled_if(disabled: bool) -> Self {
        if disabled {
            Self::Disabled
        } else {
            Self::Unselected
        }
    }

    fn selected_if(selected: bool) -> Self {
        if selected {
            Self::Selected
        } else {
            Self::Unselected
        }
    }

    fn or_else<F>(self, f: F) -> Self
    where
        F: FnOnce() -> Self,
    {
        match self {
            Self::Unselected => f(),
            _ => self,
        }
    }
}

impl BitOr for ButtonState {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Self::Unselected => rhs,
            _ => self,
        }
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self::Unselected
    }
}
