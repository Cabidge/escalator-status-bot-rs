use crate::data::{EscalatorInput, InputError};

#[derive(Debug, poise::Modal)]
#[name = "Report an Escalator"]
pub struct ReportModal {
    #[name = "Which Escalator(s)?"]
    #[placeholder = "(eg. 4-2 for the \"4 to 2\")"]
    #[min_length = 3]
    #[max_length = 3]
    escalator: String,
}

impl TryFrom<ReportModal> for EscalatorInput {
    type Error = InputError;

    fn try_from(value: ReportModal) -> Result<Self, Self::Error> {
        value.escalator.parse::<EscalatorInput>()
    }
}
