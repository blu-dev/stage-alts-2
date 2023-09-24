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

fn dlc_stages() -> [Hash40; 11] {
    [
        Hash40::from("jack_mementoes"),
        Hash40::from("brave_altar"),
        Hash40::from("buddy_spiral"),
        Hash40::from("dolly_stadium"),
        Hash40::from("fe_shrine"),
        Hash40::from("tantan_spring"),
        Hash40::from("pickel_world"),
        Hash40::from("ff_cave"),
        Hash40::from("xeno_alst"),
        Hash40::from("demon_dojo"),
        Hash40::from("trail_castle"),
    ]
}

#[derive(Copy, Clone, Debug)]
pub enum StageKind {
    Training,
    Battlefield,
    SmallBattlefield,
    BigBattlefield,
    FinalDestination,

    Normal(Hash40),
    DLC(Hash40),
}

impl StageKind {
    pub fn as_hash(&self) -> Hash40 {
        match self {
            Self::Training => Hash40::from("training"),
            Self::Battlefield => Hash40::from("battlefield"),
            Self::SmallBattlefield => Hash40::from("battlefield_s"),
            Self::BigBattlefield => Hash40::from("battlefield_l"),
            Self::FinalDestination => Hash40::from("end"),
            Self::Normal(normal) => *normal,
            Self::DLC(dlc) => *dlc,
        }
    }
}

impl From<Hash40> for StageKind {
    fn from(value: Hash40) -> Self {
        if value == Hash40::from("training") {
            Self::Training
        } else if value == Hash40::from("battlefield") {
            Self::Battlefield
        } else if value == Hash40::from("battlefield_s") {
            Self::SmallBattlefield
        } else if value == Hash40::from("battlefield_l") {
            Self::BigBattlefield
        } else if value == Hash40::from("end") {
            Self::FinalDestination
        } else if dlc_stages().contains(&value) {
            Self::DLC(value)
        } else {
            Self::Normal(value)
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct UiPaths {
    pub normal: Hash40,
    pub battle: Hash40,
    pub end: Hash40,
}

impl UiPaths {
    pub fn new(value: StageKind, alt_id: usize) -> Self {
        let extension = if alt_id == 0 {
            Hash40::from(".bntx")
        } else {
            Hash40::from(format!("_s{alt_id:02}.bntx").as_str())
        };

        let normal = match value {
            kind @ (StageKind::DLC(_) | StageKind::SmallBattlefield) => {
                Hash40::from("ui/replace_patch/stage/stage_2/stage_2_")
                    .concat(kind.as_hash())
                    .concat(extension)
            }
            kind => Hash40::from("ui/replace/stage/stage_2/stage_2_")
                .concat(kind.as_hash())
                .concat(extension),
        };

        let battle = match value {
            StageKind::Training => normal,
            StageKind::Battlefield | StageKind::BigBattlefield | StageKind::SmallBattlefield => {
                Hash40::from("ui/replace/stage/stage_2/stage_2_battlefield").concat(extension)
            }
            StageKind::DLC(hash) => Hash40::from("ui/replace_patch/stage/stage_4/stage_4_")
                .concat(hash)
                .concat(extension),
            other => Hash40::from("ui/replace/stage/stage_4/stage_4_")
                .concat(other.as_hash())
                .concat(extension),
        };

        let end = match value {
            StageKind::Training | StageKind::FinalDestination => normal,
            StageKind::Battlefield | StageKind::BigBattlefield | StageKind::SmallBattlefield => {
                Hash40::from("ui/replace/stage/stage_3/stage_3_battlefield_")
                    .concat(format!("s{alt_id:02}.bntx").as_str())
            }
            StageKind::DLC(hash) => Hash40::from("ui/replace_patch/stage/stage_4/stage_3_")
                .concat(hash)
                .concat(extension),
            other => Hash40::from("ui/replace/stage/stage_3/stage_3_")
                .concat(other.as_hash())
                .concat(extension),
        };

        Self {
            normal,
            battle,
            end,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AltInfo {
    pub slot_value: usize,
    pub wifi_safe: bool,
    pub ui_paths: UiPaths,
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

    // For lua
    pub index_to_hash: BTreeMap<usize, Hash40>,
    pub ui_to_place: BTreeMap<Hash40, Hash40>,
}

impl AltManager {
    pub const fn new() -> Self {
        Self {
            alts: BTreeMap::new(),
            selected_alts: None,
            backup_filepaths: BTreeMap::new(),
            backup_searchpaths: BTreeMap::new(),
            index_to_hash: BTreeMap::new(),
            ui_to_place: BTreeMap::new(),
        }
    }

    pub fn add_alt(&mut self, stage_info: StageInfo, alt: usize, kind: StageKind) {
        self.alts.entry(stage_info).or_default().push(AltInfo {
            slot_value: alt,
            wifi_safe: true,
            ui_paths: UiPaths::new(kind, alt),
        });
    }

    pub fn nth_alt(&self, info: StageInfo, index: usize) -> Option<AltInfo> {
        if index == 0 {
            return None;
        }

        self.alts
            .get(&info)
            .and_then(|list| list.get(index - 1))
            .copied()
    }

    pub fn set_alts(
        &mut self,
        first: SelectedAltInfo,
        second: Option<SelectedAltInfo>,
        third: Option<SelectedAltInfo>,
    ) {
        let playable = match (second, third) {
            (Some(second), Some(third)) => PlayableAlts::ThreeStages([first, second, third]),
            (Some(second), None) => PlayableAlts::TwoStages([first, second]),
            (None, Some(third)) => PlayableAlts::TwoStages([first, third]),
            (None, None) => PlayableAlts::OneStage(first),
        };

        self.selected_alts = Some(SelectedAlts {
            playable,
            current_index: 0,
        });
    }

    pub fn fetch_advance(&mut self) -> Option<usize> {
        let alts = self.selected_alts.as_mut()?;
        match alts.playable {
            PlayableAlts::OneStage(info) => self
                .nth_alt(info.stage_info, info.index)
                .map(|info| info.slot_value),
            PlayableAlts::TwoStages(infos) => {
                let info = infos[alts.current_index % 2];
                alts.current_index += 1;
                self.nth_alt(info.stage_info, info.index)
                    .map(|info| info.slot_value)
            }
            PlayableAlts::ThreeStages(infos) => {
                let info = infos[alts.current_index % 3];
                alts.current_index += 1;
                self.nth_alt(info.stage_info, info.index)
                    .map(|info| info.slot_value)
            }
        }
    }
}
