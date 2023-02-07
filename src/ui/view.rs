use std::{fmt::Display, str::FromStr};

use itertools::Itertools;

use crate::{generate, prelude::*};

use super::timeout::TimeoutSleeper;

#[derive(Clone, Debug, Default)]
pub struct ViewBuilder(View);

#[derive(Clone, Debug, Default)]
pub struct View {
    pub content: String,
    pub rows: ActionView,
}

#[derive(Clone, Debug, Default)]
pub struct ActionView {
    action_rows: Vec<serenity::CreateActionRow>,
    scroll: ActionScroll,
}

#[derive(Clone, Copy, Debug)]
pub enum ActionScroll {
    Offset(usize),
    Page(usize),
}

pub const SCROLL_ID_PREFIX: &str = "$SCROLL-";
pub const OFFSET_ID_PREFIX: &str = "OFFSET-";
pub const PAGE_ID_PREFIX: &str = "PAGE-";

#[derive(Clone, Debug, Default)]
pub struct ButtonRow(Vec<serenity::CreateButton>);

impl ViewBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_content(s: impl ToString) -> Self {
        Self(View::with_content(s))
    }

    pub fn with_timeout(sleeper: &TimeoutSleeper) -> Self {
        ViewBuilder::with_content(generate::timeout_message(sleeper.time_left()))
    }

    pub fn build(self) -> View {
        self.0
    }

    pub fn add_content(&mut self, s: &str) -> &mut Self {
        if !self.0.content.is_empty() {
            self.0.content.push('\n');
        }

        self.0.content.push_str(s);

        self
    }

    pub fn scroll(&mut self, scroll: ActionScroll) -> &mut Self {
        self.0.rows.scroll = scroll;
        self
    }

    pub fn add_button(&mut self, button: serenity::CreateButton) -> &mut Self {
        self.row(|row| row.add_button(button))
    }

    pub fn add_buttons(
        &mut self,
        buttons: impl IntoIterator<Item = serenity::CreateButton>,
    ) -> &mut Self {
        for button_row in buttons.into_iter().chunks(5).into_iter() {
            self.row(|row| {
                for button in button_row {
                    row.add_button(button);
                }

                row
            });
        }

        self
    }

    pub fn button<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut serenity::CreateButton) -> &mut serenity::CreateButton,
    {
        self.row(|row| row.create_button(f))
    }

    pub fn buttons<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut ButtonRow) -> &mut ButtonRow,
    {
        let mut buttons = ButtonRow::default();
        f(&mut buttons);

        self.add_buttons(buttons.0)
    }

    pub fn add_select(&mut self, select: serenity::CreateSelectMenu) -> &mut Self {
        self.row(|row| row.add_select_menu(select))
    }

    pub fn select<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut serenity::CreateSelectMenu) -> &mut serenity::CreateSelectMenu,
    {
        self.row(|row| row.create_select_menu(f))
    }

    fn add_row(&mut self, action_row: serenity::CreateActionRow) -> &mut Self {
        self.0.rows.action_rows.push(action_row);
        self
    }

    pub fn row<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut serenity::CreateActionRow) -> &mut serenity::CreateActionRow,
    {
        let mut row = serenity::CreateActionRow::default();
        f(&mut row);

        self.add_row(row)
    }
}

impl View {
    pub fn with_content(s: impl ToString) -> Self {
        View {
            content: s.to_string(),
            ..Default::default()
        }
    }
}

impl From<ActionView> for serenity::CreateComponents {
    fn from(value: ActionView) -> Self {
        let count = value.action_rows.len();

        if count <= 5 {
            let mut components = serenity::CreateComponents::default();
            components.set_action_rows(value.action_rows);

            return components;
        }

        let offset = value.scroll.offset();

        let mut components = serenity::CreateComponents::default();
        for row in value.action_rows.into_iter().skip(offset).take(4) {
            components.add_action_row(row);
        }

        let at_start = offset == 0;
        let at_end = offset + 4 >= count;
        match value.scroll {
            ActionScroll::Offset(_) => {
                let prev = if at_start { offset } else { offset - 1 };
                let next = if at_end { offset } else { offset + 1 };

                components.create_action_row(|row| {
                    row.create_button(|button| {
                        button
                            .custom_id(ActionScroll::Offset(prev))
                            .label("▲")
                            .style(serenity::ButtonStyle::Primary)
                            .disabled(at_start)
                    })
                    .create_button(|button| {
                        button
                            .custom_id(ActionScroll::Offset(next))
                            .label("▼")
                            .style(serenity::ButtonStyle::Primary)
                            .disabled(at_end)
                    })
                });
            }
            ActionScroll::Page(page) => {
                let prev = if at_start { page } else { page - 1 };
                let next = if at_end { page } else { page + 1 };

                components.create_action_row(|row| {
                    row.create_button(|button| {
                        button
                            .custom_id(ActionScroll::Page(prev))
                            .label("◄")
                            .style(serenity::ButtonStyle::Primary)
                            .disabled(at_start)
                    })
                    .create_button(|button| {
                        button
                            .custom_id("$PAGE")
                            .label(page + 1)
                            .style(serenity::ButtonStyle::Secondary)
                            .disabled(true)
                    })
                    .create_button(|button| {
                        button
                            .custom_id(ActionScroll::Page(next))
                            .label("►")
                            .style(serenity::ButtonStyle::Primary)
                            .disabled(at_end)
                    })
                });
            }
        }

        components
    }
}

impl Display for ActionScroll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{SCROLL_ID_PREFIX}")?;

        match self {
            Self::Offset(offset) => write!(f, "{OFFSET_ID_PREFIX}{offset}"),
            Self::Page(page) => write!(f, "{PAGE_ID_PREFIX}{page}"),
        }
    }
}

impl FromStr for ActionScroll {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let scroll = s
            .strip_prefix(SCROLL_ID_PREFIX)
            .ok_or_else(|| anyhow::anyhow!("Missing scroll prefix"))?;

        if let Some(offset) = scroll.strip_prefix(OFFSET_ID_PREFIX) {
            return Ok(Self::Offset(offset.parse()?));
        }

        if let Some(page) = scroll.strip_prefix(PAGE_ID_PREFIX) {
            return Ok(Self::Page(page.parse()?));
        }

        anyhow::bail!("Does not match any prefix")
    }
}

impl ButtonRow {
    pub fn button<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut serenity::CreateButton) -> &mut serenity::CreateButton,
    {
        let mut button = serenity::CreateButton::default();
        f(&mut button);

        self.add_button(button)
    }

    pub fn add_button(&mut self, button: serenity::CreateButton) -> &mut Self {
        self.0.push(button);
        self
    }
}

impl ActionScroll {
    pub fn offset(self) -> usize {
        match self {
            Self::Offset(offset) => offset,
            Self::Page(page) => page * 4,
        }
    }
}

impl Default for ActionScroll {
    fn default() -> Self {
        ActionScroll::Offset(0)
    }
}
