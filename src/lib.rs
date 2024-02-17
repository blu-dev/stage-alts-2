use std::{
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use locks::Mutex;
use log::LevelFilter;
use logger::StageAltsLogger;
use patching::*;
use resources::types::{FilesystemInfo, LoadedDirectory, ResServiceNX};
use skyline::hooks::InlineCtx;
use smash_arc::{ArcLookup, Hash40, SearchLookup};
use smashnet::curl::Curler;
use utils::ConcatHash;

mod callbacks;
mod logger;
mod lua;
mod manager;
mod music_fix;
mod patching;
mod resources;
mod search;
mod utils;

extern "C" {
    fn initial_loading(ctx: &mut skyline::hooks::InlineCtx);
}

/// Checks if the hashes file has been downloaded, if it has not been downloaded then we will
/// download it.
///
/// This file is required fixing the search section to be in alphabetical order
fn check_download_hashes() {
    //     if Path::new("sd:/ultimate/stage-alts/Hashes_all").exists() {
    //         log::info!("Hashes file exists, no need to redownload it");
    //         return;
    //     }
    //
    //     std::fs::create_dir_all("sd:/ultimate/stage-alts").unwrap();
    //
    //     if let Err(e) = Curler::new().download(
    //         "https://raw.githubusercontent.com/ultimate-research/archive-hashes/master/Hashes_all"
    //             .to_string(),
    //         "sd:/ultimate/stage-alts/Hashes_all".to_string(),
    //     ) {
    //         log::error!("Failed to download hashes: {e:?}");
    //         panic!("{e:?}");
    //     }
}

#[skyline::hook(replace = initial_loading)]
unsafe fn initial_loading_hook(ctx: &mut skyline::hooks::InlineCtx) {
    call_original!(ctx);

    // We sort folder contents recursively because, while impossible to really
    // guarantee alphabetical sort at runtime since they are numerical hashes,
    // when the search fs is compiled the contents are retrieved alphabetically
    // and ordered that way. Looking at a dump of the unhashed search fs confirms that.
    //
    // We sort because the game will look for the first file with an extension in a folder
    // quite a few times, for example LVDs. By sorting using the file name's cracked hash string,
    // we are able to get vanilla behavior/consistent behavior. if we don't do this, then
    // on stage alts there might by random spawn issues on stage alts for stages like
    // PS2 because arcropolis has random ordering with hashsets when it builds new directories
    let lookup = search::get_search_lookup();
    search::sort_folder_contents(
        Hash40::from("/"),
        FilesystemInfo::instance_mut().unwrap().search_mut(),
        &lookup,
    );

    // We can do this before we sort, but I like doing it after. We build a lookup
    // of our alts that we will use to provide UI to the lua file
    let alts = search::build_alt_lookups();

    let fs = FilesystemInfo::instance().unwrap();

    // We lazily initialize our manager here, we don't use OnceCell because that
    // doesn't work on console
    let mut mgr = manager::MANAGER.write();

    mgr.alts = alts;

    // We backup the filepaths for restoring stage infos on each reload before potentially patching again
    mgr.backup_filepaths = fs
        .arc()
        .get_file_paths()
        .iter()
        .map(|fp| (fp.path.hash40(), fp.path.index()))
        .collect();

    // Same as above
    mgr.backup_searchpaths = fs
        .search()
        .get_path_to_index()
        .iter()
        .map(|hi| (hi.hash40(), hi.index()))
        .collect();
}

static ALT_NUMBER: Mutex<Option<usize>> = Mutex::new(None);
static IS_ONLINE: AtomicBool = AtomicBool::new(false);

#[skyline::hook(offset = 0x3540AB0)]
unsafe fn init_loaded_dir(info: &'static FilesystemInfo, index: u32) -> *mut LoadedDirectory {
    // The index will either be an index to a DirInfo (what we want) or a DirectoryOffset
    // (what we don't want)
    //
    // The indexes never overlap, so we can safely get a dir info this way for checking
    let Some(dir) = info.arc().get_dir_infos().get(index as usize) else {
        return call_original!(info, index);
    };

    let path = dir.path;

    // If the path is a descendant of stage and is NOT stage/common,
    // we should restore all files before performing our filesystem patching
    if search::is_descendant_of(path.hash40(), Hash40::from("stage"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/common"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage_jack"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage_edge"))
    {
        // "pretty" hash gives us a segmented list of hash path segments that we
        // can use to ensure that we are an immediate descendant of a
        // `stage/<stage name>` folder
        let pretty = path.hash40().pretty();
        if path.hash40().pretty().components().len() == 3 {
            let fs = FilesystemInfo::instance_mut().unwrap();

            restore_dir_info(fs.arc_mut(), pretty.sub_range(2));
            restore_search_section(fs.search_mut(), pretty.sub_range(2));
        }
    }

    let result = call_original!(info, index);

    if result.is_null() {
        return std::ptr::null_mut();
    }

    let loaded_directory = &mut *result;

    // Again, ensure that we are a stage folder that is not stage/common
    if search::is_descendant_of(path.hash40(), Hash40::from("stage"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/common"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage_jack"))
        && !search::is_descendant_of(path.hash40(), Hash40::from("stage/resultstage_edge"))
    {
        // TODO: Change this to using the alt manager
        let Some(alt) = *ALT_NUMBER.lock() else {
            return result;
        };

        let res_service = ResServiceNX::instance().unwrap();
        let fs = FilesystemInfo::instance_mut().unwrap();

        // Same criteria for restoring our filesystem, we ensure that we are the top level
        // stage form folder, as the patching method is recursive
        if path.hash40().pretty().components().len() == 3 && alt != 0 {
            patch_dir_info(fs.arc_mut(), path.hash40().pretty().sub_range(2), alt);
            patch_search_section(fs.search_mut(), path.hash40().pretty().sub_range(2), alt);
        }

        // If our alt is non-zero, then we should clear the children and add
        // add the proper children, this is more of a safeguard to ensure that
        // our filesystem changes get picked up although technically it is not
        if alt != 0 {
            log::info!("Loading {}", path.hash40().pretty());
            let arc = fs.arc();

            let files = search::collect_files_from_path(arc, fs.search(), path.hash40(), alt, true);

            for child in loaded_directory.child_path_indices.iter().copied() {
                resources::decrement_ref_count(fs, child);
            }

            loaded_directory.child_path_indices.clear();

            for file in files {
                loaded_directory.child_path_indices.push(file);
                resources::increment_ref_count(fs, file);
                resources::add_to_resource_list(res_service, file, 0);
            }
        }
    }

    result
}

#[skyline::hook(offset = 0x25fdf38, inline)]
unsafe fn prepare_for_load(ctx: &InlineCtx) {
    if IS_ONLINE.load(Ordering::Acquire) {
        return;
    }

    let search = FilesystemInfo::instance().unwrap().search();

    let Ok(path) = search.get_path_list_entry_from_hash(*ctx.registers[8].x.as_ref()) else {
        log::warn!(
            "Failed to get the path list entry from {:#x}",
            *ctx.registers[8].x.as_ref()
        );
        return;
    };

    let Ok(parent_path) = search.get_path_list_entry_from_hash(path.parent.hash40()) else {
        log::warn!(
            "Failed to get parent of the path {:#x}",
            *ctx.registers[8].x.as_ref()
        );
        return;
    };

    let mut mgr = manager::MANAGER.write();
    *ALT_NUMBER.lock() = mgr.fetch_advance();
}

unsafe fn get_place_id(stage_id: usize) -> usize {
    let start = (skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8)
        .add(0x45499b8);

    let stage_entry = start.add(stage_id * 0x48);
    let place_id = stage_entry.add(0x3c) as *const u32;
    *place_id as usize
}

unsafe fn get_place_hash(place_id: usize) -> hash40::Hash40 {
    let start = (skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8)
        .add(0x4548420);

    let stage_place_entry = start.add(place_id * 0x28) as *const u64;
    let hash = *stage_place_entry;

    hash40::Hash40(hash)
}

#[skyline::hook(offset = 0x16b9eb4, inline)]
unsafe fn fetch_current_alt_from_bgm_id(ctx: &InlineCtx) {
    let bgm_id_ptr = *ctx.registers[1].x.as_ref() + 0x28;

    let bgm_id_ptr = bgm_id_ptr as *mut u64;

    let bgm_id = *bgm_id_ptr;
    let bgm_hash = bgm_id & 0xFF_FFFFFFFF;

    let mgr = manager::MANAGER.read();
    let cache = mgr.music_cache.as_ref().unwrap();
    let stage_id = *(*ctx.registers[1].x.as_ref() as *const u32) as usize;

    let hash = get_place_hash(get_place_id(stage_id));
    if !cache.is_song_allowed(hash40::Hash40(bgm_hash)) {
        let new_song = cache.get_random_song(hash);

        *bgm_id_ptr = (*bgm_id_ptr & 0xFFFFFF00_00000000) | new_song.0;
    }

    let alt_id = (bgm_id >> 40) & 0xFFFF;
    *ALT_NUMBER.lock() = mgr.fetch_alt_for_stage(smash_arc::Hash40(hash.0), alt_id as usize);
}

#[skyline::hook(offset = 0x22d9e70, inline)]
unsafe fn online_melee_any_scene_create(_: &InlineCtx) {
    IS_ONLINE.store(true, Ordering::Release);
}

#[skyline::hook(offset = 0x22D9DA0, inline)]
unsafe fn bg_matchmaking_seq(_: &InlineCtx) {
    IS_ONLINE.store(true, Ordering::Release);
}

#[skyline::hook(offset = 0x22D9CD0, inline)]
unsafe fn arena_seq(_: &InlineCtx) {
    IS_ONLINE.store(true, Ordering::Release);
}

#[skyline::hook(offset = 0x235A62C, inline)]
unsafe fn main_menu(_: &InlineCtx) {
    IS_ONLINE.store(false, Ordering::Release);
}

#[skyline::main(name = "stage-alts")]
pub fn main() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let err_msg = format!("thread has panicked at '{}', {}", msg, location);
        skyline::error::show_error(
            69,
            "Skyline plugin as panicked! Please open the details and send a screenshot to the developer, then close the game.\n",
            err_msg.as_str()
        );
    }));

    // Initialize our logger
    log::set_logger(Box::leak(Box::new(StageAltsLogger::new()))).unwrap();
    log::set_max_level(LevelFilter::Trace);

    utils::init_hash_lookup(false);

    check_download_hashes();

    skyline::install_hooks!(
        initial_loading_hook,
        prepare_for_load,
        init_loaded_dir,
        fetch_current_alt_from_bgm_id,
        online_melee_any_scene_create,
        bg_matchmaking_seq,
        arena_seq,
        main_menu
    );

    callbacks::install();

    lua::install();
}
