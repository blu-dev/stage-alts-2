use std::collections::BTreeMap;

use locks::RwLock;
use smash_arc::{FilePath, Hash40, HashToIndex};

use crate::utils::ConcatHash;

pub static MANAGER: RwLock<AltManager> = RwLock::new(AltManager::new());

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub struct StageInfo {
    pub name: Hash40,
    pub normal_form: bool,
}

impl StageInfo {
    pub fn from_path(hash: Hash40) -> Option<Self> {
        let pretty = hash.pretty();

        let components = pretty.components();
        if components.len() != 3 {
            return None;
        }

        if components[0] != Hash40::from("stage") {
            return None;
        }

        let name = components[1];

        let normal_form = if components[2] == Hash40::from("normal") {
            true
        } else if components[2] == Hash40::from("battle") {
            false
        } else {
            return None;
        };

        Some(Self { name, normal_form })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AltInfo {
    pub slot_value: usize,
    pub wifi_safe: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct SelectedAltInfo {
    pub index: usize,
    pub stage_info: StageInfo,
}

impl Default for SelectedAltInfo {
    fn default() -> Self {
        Self {
            index: 0,
            stage_info: StageInfo {
                name: Hash40::from("battlefield"),
                normal_form: true,
            },
        }
    }
}

pub enum PlayableAlts {
    OneStage(SelectedAltInfo),
    TwoStages([SelectedAltInfo; 2]),
    ThreeStages([SelectedAltInfo; 3]),
}

pub struct SelectedAlts {
    pub playable: PlayableAlts,
    pub current_index: usize,
}

pub struct AltManager {
    pub alts: BTreeMap<StageInfo, Vec<AltInfo>>,
    pub selected_alts: Option<SelectedAlts>,

    pub backup_filepaths: BTreeMap<Hash40, u32>,
    pub backup_searchpaths: BTreeMap<Hash40, u32>,
}

impl AltManager {
    pub const fn new() -> Self {
        Self {
            alts: BTreeMap::new(),
            selected_alts: None,
            backup_filepaths: BTreeMap::new(),
            backup_searchpaths: BTreeMap::new(),
        }
    }

    pub fn add_alt(&mut self, stage_info: StageInfo, alt: usize) {
        self.alts.entry(stage_info).or_default().push(AltInfo {
            slot_value: alt,
            wifi_safe: true,
        });
    }

    pub fn nth_alt(&self, info: StageInfo, index: usize) -> Option<AltInfo> {
        self.alts
            .get(&info)
            .and_then(|list| list.get(index))
            .copied()
    }

    pub fn reset_alts(&mut self, count: usize) {
        let playable = match count {
            1 => PlayableAlts::OneStage(Default::default()),
            2 => PlayableAlts::TwoStages(Default::default()),
            3 => PlayableAlts::ThreeStages(Default::default()),
            _ => panic!("Unsupported alt count: {count}"),
        };

        self.selected_alts = Some(SelectedAlts {
            playable,
            current_index: 0,
        });
    }

    pub fn set_alt(&mut self, alt_index: usize, stage: StageInfo, alt: usize) {
        let Some(selected) = self.selected_alts.as_mut() else {
            log::error!("No alts set up when setting alt");
            return;
        };

        let selection = SelectedAltInfo {
            index: alt,
            stage_info: stage,
        };

        match (&mut selected.playable, alt_index) {
            (PlayableAlts::OneStage(info), 0) => *info = selection,
            (PlayableAlts::TwoStages(infos), idx @ 0..=1) => infos[idx] = selection,
            (PlayableAlts::ThreeStages(infos), idx @ 0..=2) => infos[idx] = selection,
            _ => log::error!("Invalid alt selection index for current playable stage count"),
        }
    }

    pub fn pick_random_alt(&mut self, selection_index: usize, stage: StageInfo) {
        let Some(alts) = self.alts.get(&stage) else {
            self.set_alt(selection_index, stage, 0);
            return;
        };

        let index = rand::random::<usize>() % alts.len();
        self.set_alt(selection_index, stage, index);
    }
}
