use std::time::{SystemTime, SystemTimeError};

use crate::{
    data::{report::UserReport, status::Status},
    prelude::*,
};

use itertools::Itertools;

pub async fn gist(pool: &sqlx::PgPool) -> Result<serenity::CreateEmbed, sqlx::Error> {
    // -- Setup

    let mut embed = serenity::CreateEmbed::default();

    embed.title("Here's the gist...");

    let escalator_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM escalators")
        .fetch_one(pool)
        .await?
        .0 as usize;

    let mut summaries = vec![];

    // add summaries for down and blocked status escalators (only if there are any of either)
    for status in [Status::Down, Status::Blocked] {
        let escalators = sqlx::query_as::<_, EscalatorFloors>(
            "
            SELECT floor_start, floor_end
            FROM escalators
            WHERE current_status = $1
            ",
        )
        .bind(status)
        .fetch_all(pool)
        .await?;

        if !escalators.is_empty() {
            summaries.push(summarize_status(status, &escalators, escalator_count));
        }
    }

    // -- Handle Variations

    if !summaries.is_empty() {
        // -- Some are Down/Blocked

        embed.color((240, 60, 60)).description(summaries.join("\n"));

        return Ok(embed);
    }

    // -- All are Open

    let emoji = Status::Open.emoji();

    embed
        .description(format!("`{emoji}` `ALL` escalators are `OPEN`! 🥳 🎉"))
        .color((55, 220, 70));

    Ok(embed)
}

/// Generates a summary for a specific status.
fn summarize_status(
    status: Status,
    escalators: &[EscalatorFloors],
    escalator_count: usize,
) -> String {
    let emoji = status.emoji();

    let mut message = format!("`{}` ", emoji);

    if escalators.len() == escalator_count {
        // all
        message.push_str("`ALL` escalators");
    } else if escalators.len() >= escalator_count / 2 {
        // more than half
        message.push_str("`MANY` escalators");
    } else {
        // less than half
        message.push_str(&nounify_escalators(escalators));
    }

    if escalators.len() == 1 {
        message.push_str(" is ");
    } else {
        message.push_str(" are ");
    }

    // TODO: make this less verbose
    let status = match status {
        Status::Open => "`OPEN`",
        Status::Down => "`DOWN`",
        Status::Blocked => "`BLOCKED`",
    };

    message.push_str(status);
    message.push('.');

    message
}

/// Turn a collection of Escalators into a format that could be put into a message.
pub fn nounify_escalators(escalators: &[EscalatorFloors]) -> String {
    if escalators.is_empty() {
        return String::from("`NO` escalators");
    }

    if escalators.len() == 1 {
        return format!("The `{}` escalator", escalators[0]);
    }

    // how many escalators there are not including the first and last
    let mid_count = escalators.len() - 2;

    let mut escalators = escalators
        .iter()
        .copied()
        .map(|escalator| format!("`{escalator}`"));
    let mut noun = format!("The {}", escalators.next().unwrap());

    for escalator in escalators.by_ref().take(mid_count) {
        noun.push_str(&format!(", {escalator}"));
    }

    noun.push_str(&format!(", and {} escalators", escalators.next().unwrap()));

    noun
}

/// Generates a message of a list of recent reports.
pub fn announcement<'a, I>(max_reports_displayed: usize, reports: I) -> String
where
    I: Iterator<Item = &'a UserReport> + ExactSizeIterator + 'a,
{
    let mut reports = reports.map(UserReport::to_string);

    if reports.len() <= max_reports_displayed {
        return reports.join("\n");
    }

    let mut message = String::new();
    for report in reports.by_ref().take(max_reports_displayed - 1) {
        message.push_str(&report);
        message.push('\n');
    }

    message.push_str(&format!("\n*(...and {} more)*", reports.len()));

    message
}

/// Generates an alert message from a user report.
pub fn alert(report: &UserReport) -> String {
    let emoji = report.new_status.emoji();
    let noun = nounify_escalators(&report.affected_escalators);
    let is_are = if report.affected_escalators.len() == 1 {
        "is"
    } else {
        "are"
    };
    let status = report.new_status.as_id_str();

    format!("`{emoji}` {noun} {is_are} `{status}`")
}

/// Generates a message containing the status of every escalator.
pub async fn menu_status(pool: &sqlx::PgPool) -> Result<String, sqlx::Error> {
    let statuses = sqlx::query_as::<_, Escalator>(
        "
        SELECT floor_start,
            floor_end,
            current_status
        FROM escalators
        ORDER BY floor_start + floor_end,
            floor_start
        ",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(Escalator::to_string)
    .chunks(2)
    .into_iter()
    .map(|mut pair| pair.join(" "))
    .join("\n");

    Ok(format!("**Escalator Statuses:**```\n{statuses}```"))
}

pub const REPORT_EMOJI: char = '📢';
pub const REPORT_BUTTON_ID: &str = "REPORT";

pub const INFO_EMOJI: char = '❔';
pub const INFO_BUTTON_ID: &str = "INFO";

pub fn menu_buttons() -> serenity::CreateComponents {
    let mut components = serenity::CreateComponents::default();

    components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.label("Report")
                .emoji(REPORT_EMOJI)
                .style(serenity::ButtonStyle::Primary)
                .custom_id(REPORT_BUTTON_ID)
        })
        .create_button(|btn| {
            btn.emoji(INFO_EMOJI)
                .style(serenity::ButtonStyle::Secondary)
                .custom_id(INFO_BUTTON_ID)
        })
    });

    components
}

/// Explanation: https://gist.github.com/LeviSnoot/d9147767abeef2f770e9ddcd91eb85aa
pub enum Timestamp {
    Default,
    ShortTime,
    LongTime,
    ShortDate,
    LongDate,
    Short,
    Long,
    Relative,
}

impl Timestamp {
    pub fn as_style(&self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::ShortTime => Some("t"),
            Self::LongTime => Some("T"),
            Self::ShortDate => Some("d"),
            Self::LongDate => Some("D"),
            Self::Short => Some("f"),
            Self::Long => Some("F"),
            Self::Relative => Some("R"),
        }
    }

    /// Generate a Discord timestamp at the given time (ie. <t:[unix_timestamp]:[style]>).
    pub fn generate_at(&self, time: SystemTime) -> Result<String, SystemTimeError> {
        let unix = time.duration_since(SystemTime::UNIX_EPOCH)?.as_secs();

        let mut timestamp = format!("<t:{unix}");

        if let Some(style) = self.as_style() {
            timestamp.push(':');
            timestamp.push_str(style);
        }

        timestamp.push('>');

        Ok(timestamp)
    }
}
