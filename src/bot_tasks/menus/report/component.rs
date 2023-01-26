use crate::prelude::*;

use super::Report;
use std::str::FromStr;

pub struct ReportComponent {

}

pub enum ComponentStatus<T> {
    Continue,
    Complete(T),
}

pub enum ComponentAction {
}

impl ReportComponent {
    pub fn new() -> Self {
        Self {

        }
    }

    pub fn render(&self) -> serenity::CreateComponents {
        todo!()
    }

    pub fn execute(&mut self, command: ComponentAction) -> ComponentStatus<Report> {
        todo!()
    }
}

impl FromStr for ComponentAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}