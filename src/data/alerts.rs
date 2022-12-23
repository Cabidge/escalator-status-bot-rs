use itertools::Itertools;
use shuttle_persist::PersistInstance;

use crate::prelude::*;

use std::collections::{HashMap, HashSet};

use super::{escalator_input::EscalatorInput, status::Status};

#[derive(Debug)]
pub struct Alerts {
    watch_lists: HashMap<serenity::User, HashSet<Escalator>>,
    should_save: bool,
}

type SimplifiedWatchlists = Vec<(u64, HashSet<Escalator>)>;

impl Alerts {
    pub async fn load_persist(persist: &PersistInstance, cache_http: impl serenity::CacheHttp) -> Self {
        let Ok(simplified) = persist.load::<SimplifiedWatchlists>("alerts") else {
            return Alerts {
                watch_lists: HashMap::new(),
                should_save: true,
            };
        };

        let mut watch_lists = HashMap::new();
        let mut should_save = false;
        for (user_id, watch_list) in simplified {
            let Ok(user) = serenity::UserId(user_id).to_user(&cache_http).await else {
                should_save = true;
                continue;
            };

            watch_lists.insert(user, watch_list);
        }

        Alerts {
            watch_lists,
            should_save,
        }
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        match self.should_save {
            true => self.should_save = false,
            false => return,
        }

        let simplified = self.watch_lists
            .iter()
            .map(|(user, watch_list)| (user.id.0, watch_list))
            .collect_vec();

        persist.save("alerts", simplified).ok();
    }

    pub fn add(&mut self, user: &serenity::User, escalator: Escalator) {
        let watch_list = match self.watch_lists.get_mut(user) {
            None => self.watch_lists.entry(user.to_owned()).or_default(),
            Some(watch_list) => watch_list,
        };

        if watch_list.insert(escalator) {
            self.should_save = true;
        }
    }

    pub fn remove(&mut self, user: &serenity::User, escalator: Escalator) {
        let Some(list) = self.watch_lists.get_mut(user) else { return };
        if list.remove(&escalator) {
            self.should_save = true;

            // completely remove list if it is now empty
            if list.is_empty() {
                self.watch_lists.remove(user);
            }
        }
    }

    pub fn get_watch_list(&self, user: &serenity::User) -> Option<&HashSet<Escalator>> {
        self.watch_lists.get(user)
    }

    pub async fn alert(
        &self,
        cache_http: impl serenity::CacheHttp,
        escalators: EscalatorInput,
        status: Status,
    ) {
        let emoji = status.emoji();
        let noun = escalators.message_noun();
        let is_are = if escalators.is_singular() {
            "is"
        } else {
            "are"
        };
        let status = status.as_id_str();
        let message = format!("`{emoji}` {noun} {is_are} `{status}`");

        for user in self.users_watching(escalators) {
            user.direct_message(&cache_http, |msg| msg.content(&message))
                .await
                .ok();
        }
    }

    fn users_watching(&self, escalators: EscalatorInput) -> impl Iterator<Item = &serenity::User> {
        self.watch_lists
            .iter()
            .filter_map(move |(user, watch_list)| {
                is_on_watchlist(watch_list, escalators).then_some(user)
            })
    }
}

fn is_on_watchlist(watch_list: &HashSet<Escalator>, escalators: EscalatorInput) -> bool {
    match escalators {
        EscalatorInput::All => !watch_list.is_empty(),
        EscalatorInput::Direct(start, end) => watch_list.contains(&(start, end)),
        EscalatorInput::Pair(a, b) => watch_list.contains(&(a, b)) || watch_list.contains(&(b, a)),
    }
}
