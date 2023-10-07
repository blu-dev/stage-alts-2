use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use hash40::{hash40, Hash40};
use prc::{ParamKind, ParamStruct};

pub struct MusicCache {
    pub allowed_songs: HashSet<Hash40>,
    pub song_by_series: HashMap<Hash40, Vec<Hash40>>,
    pub stage_to_series: HashMap<Hash40, Hash40>,
}

fn prc_get(prc: &ParamStruct, key: Hash40) -> Option<&ParamKind> {
    prc.0.iter().find_map(|(k, v)| (*k == key).then_some(v))
}

fn collect_stage_name_to_bgm_set(prc: &ParamStruct) -> HashMap<Hash40, Hash40> {
    let (_, main_list) = &prc.0[0];

    let ParamKind::List(list) = main_list else {
        unreachable!()
    };

    let mut map = HashMap::with_capacity(list.0.len());

    for param in list.0.iter() {
        let ParamKind::Struct(stage) = param else {
            continue;
        };

        let stage_place_id = prc_get(stage, hash40("stage_place_id"))
            .and_then(|param| {
                let ParamKind::Hash(hash) = param else {
                    return None;
                };
                Some(*hash)
            })
            .unwrap();
        let bgm_set_id = prc_get(stage, hash40("bgm_set_id"))
            .and_then(|param| {
                let ParamKind::Hash(hash) = param else {
                    return None;
                };
                Some(*hash)
            })
            .unwrap();

        map.insert(stage_place_id, bgm_set_id);
    }

    map
}

fn collect_bgm_lists(prc: &ParamStruct) -> HashMap<Hash40, Vec<Hash40>> {
    let mut map = HashMap::new();
    // skip 6 to start iterating over the lists
    for (k, v) in prc.0.iter().skip(6) {
        let ParamKind::List(list) = v else {
            continue;
        };

        let songs: Vec<_> = list
            .0
            .iter()
            .filter_map(|param| {
                let ParamKind::Struct(param) = param else {
                    return None;
                };

                prc_get(param, hash40("ui_bgm_id")).and_then(|param| {
                    let ParamKind::Hash(hash) = param else {
                        return None;
                    };

                    Some(*hash)
                })
            })
            .collect();

        map.insert(*k, songs);
    }

    map
}

impl MusicCache {
    pub fn new(stage: &[u8], music: &[u8]) -> Self {
        let stage = prc::read_stream(&mut std::io::Cursor::new(stage)).unwrap();
        let bgm = prc::read_stream(&mut std::io::Cursor::new(music)).unwrap();

        let name_to_set = collect_stage_name_to_bgm_set(&stage);
        let bgm_lists = collect_bgm_lists(&bgm);

        let all_songs: HashSet<Hash40> = bgm_lists
            .values()
            .flat_map(|bgm_set_list| bgm_set_list.iter().copied())
            .collect();

        Self {
            allowed_songs: all_songs,
            song_by_series: bgm_lists,
            stage_to_series: name_to_set,
        }
    }

    pub fn is_song_allowed(&self, hash: Hash40) -> bool {
        self.allowed_songs.contains(&hash)
    }

    pub fn get_random_song(&self, stage_name: Hash40) -> Hash40 {
        use rand::prelude::*;

        let Some(series) = self.stage_to_series.get(&stage_name) else {
            return hash40("ui_bgm_crs2_02_senjyou");
        };

        let Some(list) = self.song_by_series.get(&series) else {
            return hash40("ui_bgm_crs2_02_senjyou");
        };

        list.choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or_else(|| hash40("ui_bgm_crs2_02_senjyou"))
    }
}
