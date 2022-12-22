use std::{ops::BitOr, sync::Arc, time::Duration};

use crate::prelude::*;

use super::{escalator_input::EscalatorInput, status::Status, Statuses, UserReport};

use anyhow::anyhow;
use poise::futures_util::StreamExt;
use shuttle_persist::PersistInstance;
use tokio::{sync::mpsc, task};

#[derive(Debug)]
pub struct ReportMenu {
    shard_messenger: serenity::ShardMessenger,
    user_reports: mpsc::Sender<UserReport>,
    menu: Option<MenuHandle>,
    should_save: bool,
}

#[derive(Debug)]
struct MenuHandle {
    message: serenity::Message,
    interaction_task: task::JoinHandle<()>,
}

impl ReportMenu {
    pub async fn load_persist(
        user_reports: mpsc::Sender<UserReport>,
        ctx: &serenity::Context,
        persist: &PersistInstance,
    ) -> Self {
        let Ok(ids) = persist.load::<Option<(u64, u64)>>("report_menu") else {
            return Self::new(ctx.shard.clone(), user_reports, true);
        };

        let Some(ids) = ids else {
            return Self::new(ctx.shard.clone(), user_reports, false);
        };

        let channel_id = serenity::ChannelId(ids.0);
        let message_id = serenity::MessageId(ids.1);

        let Ok(message) = channel_id.message(ctx, message_id).await else {
            return Self::new(ctx.shard.clone(), user_reports, true);
        };

        let mut report_menu = Self::new(ctx.shard.clone(), user_reports, false);
        let menu = report_menu.create_handle(Arc::clone(&ctx.http), message);

        report_menu.menu = Some(menu);

        report_menu
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let ids = self.menu.as_ref().map(MenuHandle::ids);

            let _ = persist.save("report_menu", ids).ok();

            self.should_save = false;
        }
    }

    fn new(
        shard_messenger: serenity::ShardMessenger,
        user_reports: mpsc::Sender<UserReport>,
        should_save: bool,
    ) -> Self {
        Self {
            menu: None,
            shard_messenger,
            user_reports,
            should_save,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.menu.is_some()
    }

    /// Tries to update the report menu message.
    ///
    /// Only returns an error if there was an error trying to edit the message.
    pub async fn update(
        &mut self,
        cache_http: impl serenity::CacheHttp,
        statuses: &Statuses,
    ) -> Result<(), Error> {
        let Some(menu) = &mut self.menu else { return Ok(()) };

        let content = statuses.menu_message();
        menu.update_message(cache_http, &content).await?;

        Ok(())
    }

    pub async fn initialize(&mut self, ctx: Context<'_>) -> Result<(), Error> {
        if self.is_initialized() {
            ctx.send(|msg| {
                msg.content("A report menu already exists, run `/menu clear` to delete it and clear it from memory.")
                    .ephemeral(true)
            }).await?;

            return Ok(());
        }

        let content = ctx.data().statuses.lock().await.menu_message();

        let handle = ctx
            .send(|msg| {
                msg.content(content)
                    .components(add_report_buttons)
                    .ephemeral(false)
            })
            .await?;

        let message = handle.into_message().await?;
        let http = Arc::clone(&ctx.serenity_context().http);

        let menu = self.create_handle(http, message);
        self.menu = Some(menu);
        self.should_save = true;

        Ok(())
    }

    pub async fn clear(&mut self, ctx: Context<'_>) -> Result<(), Error> {
        let Some(menu) = self.menu.take() else {
            ctx.say("Report menu is already cleared.").await?;
            return Ok(());
        };

        self.should_save = true;

        menu.delete(ctx).await;

        ctx.say("Report menu successfully cleared from memory.")
            .await?;

        Ok(())
    }

    fn create_handle(&self, http: Arc<serenity::Http>, message: serenity::Message) -> MenuHandle {
        let mut collector = message
            .await_component_interactions(&self.shard_messenger)
            .build();
        let user_reports = self.user_reports.downgrade();
        let shard = self.shard_messenger.clone();

        let interaction_task = tokio::spawn(async move {
            while let Some(interaction) = collector.next().await {
                let Some(user_reports) = user_reports.upgrade() else {
                    break;
                };

                // TODO: log error
                if let Err(err) =
                    handle_interaction(&interaction, user_reports, &http, &shard).await
                {
                    println!("{err:?}");
                }
            }
        });

        MenuHandle {
            message,
            interaction_task,
        }
    }
}

impl MenuHandle {
    async fn delete(self, cache_http: impl serenity::CacheHttp) {
        self.interaction_task.abort();
        let _ = self.message.delete(cache_http).await.ok();
    }

    async fn update_message(
        &mut self,
        cache_http: impl serenity::CacheHttp,
        content: &str,
    ) -> Result<(), Error> {
        self.message
            .edit(cache_http, |msg| msg.content(content))
            .await?;

        Ok(())
    }

    fn ids(&self) -> (u64, u64) {
        (self.message.channel_id.0, self.message.id.0)
    }
}

impl Drop for MenuHandle {
    fn drop(&mut self) {
        self.interaction_task.abort();
    }
}

pub const REPORT_EMOJI: char = '📢';
pub const REPORT_BUTTON_ID: &str = "REPORT";

/// Generate report button components with the proper IDs.
pub fn add_report_buttons(
    components: &mut serenity::CreateComponents,
) -> &mut serenity::CreateComponents {
    components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.label("REPORT")
                .emoji(REPORT_EMOJI)
                .style(serenity::ButtonStyle::Secondary)
                .custom_id(REPORT_BUTTON_ID)
        })
    })
}

#[derive(Default, Debug, Clone, Copy)]
struct ReportComponents {
    escalators: EscalatorComponents,
    status: Option<Status>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscalatorComponents {
    Floors {
        floors: Option<(u8, Option<u8>)>,
        pair: bool,
    },
    All,
}

async fn handle_interaction(
    interaction: &serenity::MessageComponentInteraction,
    user_reports: mpsc::Sender<UserReport>,
    http: &serenity::Http,
    shard: &serenity::ShardMessenger,
) -> Result<(), Error> {
    if interaction.data.custom_id != REPORT_BUTTON_ID {
        return Ok(());
    }

    let mut report_input = ReportComponents::default();

    interaction
        .create_interaction_response(http, |res| {
            res.interaction_response_data(|data| {
                data.set_components(report_input.create_components())
                    .ephemeral(true)
            })
        })
        .await?;

    let message = interaction.get_interaction_response(http).await?;

    let res = loop {
        let sleep = tokio::time::sleep(Duration::from_secs(2 * 60));
        tokio::pin!(sleep);

        let action = tokio::select! {
            Some(action) = message.await_component_interaction(shard) => action,
            _ = sleep => break Err(anyhow!("Timeout")),
        };

        if let Some(report) = report_input.try_action(&action.data.custom_id) {
            break Ok(report);
        }

        action.defer(http).await.ok();
        interaction
            .edit_original_interaction_response(http, |res| {
                res.components(|components| {
                    *components = report_input.create_components();
                    components
                })
            })
            .await
            .ok();
    };

    let Ok(mut report) = res else {
        interaction.edit_original_interaction_response(http, |msg| {
            msg.content("Interaction timed out, try again...")
                .components(|components| {
                    components.set_action_rows(vec![])
                })
        }).await.ok();
        return Ok(());
    };

    if let Some(member) = &interaction.member {
        report.reporter = Some(member.user.id);
    }

    // TODO: log error
    if let Err(err) = user_reports.send(report).await {
        println!("{err}");
    }

    let message = format!(
        "{} Successfully reported {}.",
        report.status.emoji(),
        report.escalators.message_noun()
    );
    interaction
        .edit_original_interaction_response(http, |msg| {
            msg.content(message)
                .components(|components| components.set_action_rows(vec![]))
        })
        .await
        .ok();

    Ok(())
}

const SELECTED_BUTTON_STYLE: serenity::ButtonStyle = serenity::ButtonStyle::Primary;
const UNSELECTED_BUTTON_STYLE: serenity::ButtonStyle = serenity::ButtonStyle::Secondary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Selected,
    Unselected,
    Disabled,
}

impl ButtonState {
    fn disabled(disabled: bool) -> Self {
        if disabled {
            Self::Disabled
        } else {
            Self::Unselected
        }
    }

    fn selected(selected: bool) -> Self {
        if selected {
            Self::Selected
        } else {
            Self::Unselected
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

fn create_button(id: &str, state: ButtonState) -> serenity::CreateButton {
    let mut button = serenity::CreateButton::default();

    match state {
        ButtonState::Selected => button.style(SELECTED_BUTTON_STYLE).disabled(false),
        ButtonState::Unselected => button.style(UNSELECTED_BUTTON_STYLE).disabled(false),
        ButtonState::Disabled => button.style(UNSELECTED_BUTTON_STYLE).disabled(true),
    };

    button.custom_id(id);

    button
}

const PAIR_BUTTON_ID: &str = "REPORT-PAIR";
const ALL_BUTTON_ID: &str = "REPORT-ALL";
const SUBMIT_BUTTON_ID: &str = "REPORT-SUBMIT";

const NUMBER_BUTTON_ID_PREFIX: &str = "REPORT-FLOOR-";
const STATUS_BUTTON_ID_PREFIX: &str = "REPORT-STATUS-";

impl ReportComponents {
    /// Tries to parse the action and update the components accordingly.
    ///
    /// If the action was a submit action and it succeeds, return a generated
    /// UserReport with the relevant information.
    fn try_action(&mut self, action_id: &str) -> Option<UserReport> {
        if action_id == PAIR_BUTTON_ID {
            self.escalators.select_pair();
            return None;
        }

        if action_id == ALL_BUTTON_ID {
            self.escalators.select_all();
            return None;
        }

        if action_id == SUBMIT_BUTTON_ID {
            return self.try_as_report();
        }

        if let Some(floor) = action_id.strip_prefix(NUMBER_BUTTON_ID_PREFIX) {
            self.escalators.select_floor(floor.parse::<u8>().ok()?);
            return None;
        }

        if let Some(status) = action_id.strip_prefix(STATUS_BUTTON_ID_PREFIX) {
            self.select_status(status.parse::<Status>().ok()?);
            return None;
        }

        None
    }

    fn try_as_report(&self) -> Option<UserReport> {
        let escalators = self.escalators.try_as_escalator_input()?;
        let status = self.status?;

        Some(UserReport {
            escalators,
            status,
            reporter: None,
        })
    }

    fn select_status(&mut self, status: Status) {
        match self.status {
            // if no status is selected, set the status
            None => self.status = Some(status),
            // if the selected status was clicked again, unset the status
            Some(old_status) if status == old_status => self.status = None,
            // otherwise, don't do anything
            Some(_) => (),
        }
    }

    fn create_components(&self) -> serenity::CreateComponents {
        let mut components = serenity::CreateComponents::default();

        // selecting escalators
        for action_row in self.escalators.create_action_rows() {
            components.add_action_row(action_row);
        }

        // selecting status
        components.add_action_row(self.create_status_action_row());

        // submit button
        components
            .create_action_row(|action_row| action_row.add_button(self.create_submit_button()));

        components
    }

    fn create_status_action_row(&self) -> serenity::CreateActionRow {
        let mut action_row = serenity::CreateActionRow::default();

        for status in [Status::Open, Status::Down, Status::Blocked] {
            let id = format!("{}{}", STATUS_BUTTON_ID_PREFIX, status.as_id_str());
            let state = ButtonState::selected(Some(status) == self.status)
                | ButtonState::disabled(self.status.is_some());

            let mut button = create_button(&id, state);

            button.emoji(status.emoji());

            action_row.add_button(button);
        }

        action_row
    }

    fn create_submit_button(&self) -> serenity::CreateButton {
        let Some(escalators) = self.escalators.try_as_escalator_input() else {
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

impl EscalatorComponents {
    fn select_pair(&mut self) {
        if let Self::Floors { pair, .. } = self {
            *pair = !*pair
        }
    }

    fn select_all(&mut self) {
        match self {
            Self::Floors { .. } => *self = Self::All,
            Self::All => *self = Self::default(),
        }
    }

    fn select_floor(&mut self, floor: u8) {
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
                    if Self::is_valid_escalator(*start, floor) {
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

    fn try_as_escalator_input(&self) -> Option<EscalatorInput> {
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

    fn is_valid_escalator(start: u8, end: u8) -> bool {
        // if the start and end are valid floors...
        ((2..=9).contains(&start) && (2..=9).contains(&end))
            // ...and the distance between the two floors is 2
            //                           ...or it is the 2-3 or 3-2
            && (start.abs_diff(end) == 2 || matches!((start, end), (2, 3) | (3, 2)))
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
                Some((start, None)) => Self::is_valid_escalator(*start, floor),
                Some((_, Some(_))) => false,
                None => true,
            },
            Self::All => false,
        }
    }

    fn create_action_rows(&self) -> Vec<serenity::CreateActionRow> {
        const NUMBER_PANEL: [[u8; 4]; 2] = [[2, 4, 6, 8], [3, 5, 7, 9]];

        let mut action_rows = vec![];
        for (row, numbers) in NUMBER_PANEL.into_iter().enumerate() {
            let mut action_row = serenity::CreateActionRow::default();

            for floor in numbers {
                let id = format!("{}{}", NUMBER_BUTTON_ID_PREFIX, floor);
                let state = ButtonState::selected(self.is_floor_selected(floor))
                    | ButtonState::disabled(!self.is_valid_next_floor(floor));

                let mut button = create_button(&id, state);
                button.label(floor);

                action_row.add_button(button);
            }

            let button = match row {
                // top row
                0 => self.create_pair_button(),
                // bottom row
                1 => self.create_all_button(),
                _ => unreachable!(),
            };

            action_row.add_button(button);

            action_rows.push(action_row);
        }

        action_rows
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

    fn create_pair_button(&self) -> serenity::CreateButton {
        let button_state =
            ButtonState::disabled(self.is_all()) | ButtonState::selected(self.is_pair());
        let mut button = create_button(PAIR_BUTTON_ID, button_state);
        button.label("Pair");

        button
    }

    fn create_all_button(&self) -> serenity::CreateButton {
        let button_state = ButtonState::selected(self.is_all());
        let mut button = create_button(ALL_BUTTON_ID, button_state);
        button.label("All");

        button
    }
}

impl Default for EscalatorComponents {
    fn default() -> Self {
        EscalatorComponents::Floors {
            floors: None,
            pair: false,
        }
    }
}
