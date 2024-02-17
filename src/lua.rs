use std::{collections::BTreeMap, io::Cursor, path::Path, ptr::NonNull};

use prc::ParamKind;
use rlua_lua53_sys as lua;
use skyline::hooks::InlineCtx;
use smash_arc::{ArcLookup, Hash40};

use crate::{
    manager::{SelectedAltInfo, StageInfo, StageKind, UiPaths, MANAGER},
    resources::{self, types::FilesystemInfo},
    utils::ConcatHash,
};

extern "C" fn send_message(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let value = skyline::from_c_str(lua::lua_tostring(state, -1) as _);
        log::info!("Lua says: {}", value);
        lua::lua_pop(state, 1);
        0
    }
}

extern "C" fn print_panel_name(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let index = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        if usize::MAX == index {
            return 0;
        }

        let Some(hash) = MANAGER.read().index_to_hash.get(&index).copied() else {
            log::warn!("No hash for index {index}");
            return 0;
        };

        log::info!("Index {index}: {}", crate::utils::string_for_hash(hash));

        0
    }
}

extern "C" fn get_panel_alt_count(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let form_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        let panel_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        if usize::MAX == panel_id {
            lua::lua_pushinteger(state, 0);
            return 1;
        }

        let Some(hash) = MANAGER.read().index_to_hash.get(&panel_id).copied() else {
            log::warn!("No hash for index {panel_id}");
            lua::lua_pushinteger(state, 0);
            return 1;
        };

        let count = MANAGER
            .read()
            .alts
            .get(&StageInfo {
                name: hash,
                normal_form: form_id == 0,
            })
            .map(|v| v.len())
            .unwrap_or_default();

        lua::lua_pushinteger(state, count as i64);
        1
    }
}

extern "C" fn get_alt_texture_index(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let alt_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        let form_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        let panel_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        if usize::MAX == panel_id {
            lua::lua_pushinteger(state, -1);
            return 1;
        }

        let Some(hash) = MANAGER.read().index_to_hash.get(&panel_id).copied() else {
            log::warn!("No hash for index {panel_id}");
            lua::lua_pushinteger(state, -1);
            return 1;
        };

        let mgr = MANAGER.read();

        let paths = if alt_id == 0 {
            UiPaths::new(StageKind::from(hash), 0)
        } else {
            let Some(alt) = mgr
                .alts
                .get(&StageInfo {
                    name: hash,
                    normal_form: form_id == 0,
                })
                .and_then(|alts| alts.get(alt_id - 1))
            else {
                lua::lua_pushinteger(state, -1);
                return 1;
            };

            alt.ui_paths
        };

        let arc = FilesystemInfo::instance().unwrap().arc();

        let path = match form_id {
            1 => paths.battle,
            2 => paths.end,
            _ => paths.normal,
        };

        let Ok(index) = arc.get_file_path_index_from_hash(path) else {
            log::warn!("Could not get file path index for {}", path.pretty());
            lua::lua_pushinteger(state, -1);
            return 1;
        };

        lua::lua_pushinteger(state, index.0 as i64);
        1
    }
}

extern "C" fn write_alt_field_to_bgm_id(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let mgr = MANAGER.write();

        let alt_id = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        let stage_preview_idx = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as usize;
        lua::lua_pop(state, 1);

        match mgr.current_singleton {
            Some(mut ptr) => {
                let object_ptr: *mut u64 = (ptr.as_mut() as *mut () as *mut u8)
                    .add(0xf8 + 0x28 * stage_preview_idx)
                    .cast();

                *object_ptr.add(2) &= 0xFF0000FF_FFFFFFFF;
                *object_ptr.add(2) |= ((alt_id & 0xFFFF) as u64) << 40;

                // let ui_hash = *object_ptr.add(2) & 0xFF_FFFFFFFF;
                // if ui_hash == hash40::hash40("ui_bgm_random").0 {
                //     let bgm_id_ptr = object_ptr.add(2);
                //     let panel = *(object_ptr.add(3) as *const u32);
                //     if let Some(hash) = mgr.index_to_hash.get(&(panel as usize)) {
                //         println!("{:#x}", hash.0);
                //         let new_song = mgr
                //             .music_cache
                //             .as_ref()
                //             .unwrap()
                //             .get_random_song(hash40::Hash40(hash.0));
                //         *bgm_id_ptr = (*bgm_id_ptr & 0xFFFFFF00_00000000) | new_song.0;
                //     }
                // }

                log::info!("Set BGM id to {:#x}", *object_ptr.add(2));
            }
            None => {
                log::error!(
                    "Failed to write the stage alt field to bgm because the current singleton does not exist"
                );
            }
        }

        0
    }
}

extern "C" fn set_alts(state: *mut lua::lua_State) -> i32 {
    unsafe {
        let mut mgr = MANAGER.write();

        let pop_info = || {
            let form = lua::lua_tointegerx(state, -1, std::ptr::null_mut());
            lua::lua_pop(state, 1);

            let panel = lua::lua_tointegerx(state, -1, std::ptr::null_mut());
            lua::lua_pop(state, 1);

            let alt = lua::lua_tointegerx(state, -1, std::ptr::null_mut());
            lua::lua_pop(state, 1);

            if form < 0 || panel < 0 || alt < 0 {
                None
            } else {
                mgr.index_to_hash
                    .get(&(panel as usize))
                    .copied()
                    .map(|name| SelectedAltInfo {
                        index: alt as usize,
                        stage_info: StageInfo {
                            name,
                            normal_form: form == 0,
                        },
                    })
            }
        };

        let third = pop_info();
        let second = pop_info();
        let first = pop_info();

        if let Some(first) = first {
            mgr.set_alts(first, second, third);
        }

        0
    }
}

unsafe fn push_new_singleton(
    lua_state: *mut lua::lua_State,
    name: &'static str,
    registry: &[lua::luaL_Reg],
) {
    let real_name = format!("{}\0", name);
    let meta_name = format!("Metatable{}\0", name);
    lua::luaL_newmetatable(lua_state, meta_name.as_ptr() as _);
    lua::lua_pushvalue(lua_state, -1);
    lua::lua_setfield(lua_state, -2, "__index\0".as_ptr() as _);

    lua::luaL_setfuncs(lua_state, registry.as_ptr(), 0);
    lua::lua_pop(lua_state, 1);

    lua::lua_newtable(lua_state);
    lua::lua_getfield(lua_state, lua::LUA_REGISTRYINDEX, meta_name.as_ptr() as _);
    lua::lua_setmetatable(lua_state, -2);

    let global_table = lua::bindings::index2addr(lua_state, lua::LUA_REGISTRYINDEX);
    let table = (*global_table).value.ptr;
    let value = if *(table as *mut u32).add(3) < 2 {
        todo!()
    } else {
        (*(table as *mut *mut lua::bindings::TValue).add(2)).add(1)
    };
    lua::bindings::auxsetstr(lua_state, value, real_name.as_ptr() as _);
}

#[skyline::hook(offset = 0x3373CC8, inline)]
unsafe fn add_to_key_context(ctx: &InlineCtx) {
    let lua_state: *mut lua::lua_State = *ctx.registers[19].x.as_ref() as _;

    let registry = &[
        lua::luaL_Reg {
            name: "send_message\0".as_ptr() as _,
            func: Some(send_message),
        },
        lua::luaL_Reg {
            name: "print_panel_name\0".as_ptr() as _,
            func: Some(print_panel_name),
        },
        lua::luaL_Reg {
            name: "get_panel_alt_count\0".as_ptr() as _,
            func: Some(get_panel_alt_count),
        },
        lua::luaL_Reg {
            name: "get_alt_texture_index\0".as_ptr() as _,
            func: Some(get_alt_texture_index),
        },
        lua::luaL_Reg {
            name: "set_alts\0".as_ptr() as _,
            func: Some(set_alts),
        },
        lua::luaL_Reg {
            name: "write_alt_field_to_bgm_id\0".as_ptr() as _,
            func: Some(write_alt_field_to_bgm_id),
        },
        lua::luaL_Reg {
            name: std::ptr::null(),
            func: None,
        },
    ];

    push_new_singleton(lua_state, "Alts", registry);
}

#[repr(C)]
struct StageEntry {
    key: u64,
    params: [f32; 4],
}

#[skyline::hook(offset = 0x1b32780)]
unsafe fn is_valid_entrance_param(arg: u64, arg2: i32) -> bool {
    let mut manager = MANAGER.write();

    manager.current_singleton = NonNull::new(arg as _);

    let vec = &mut *((arg + 0x168) as *mut resources::containers::CppVector<StageEntry>);

    manager.index_to_hash.clear();

    for (index, entry) in vec.iter().enumerate() {
        let hash = Hash40(entry.key & 0xFF_FFFF_FFFF);
        let Some(place) = manager.ui_to_place.get(&hash).copied() else {
            log::warn!("Failed to find place id for ui {:#x}", hash.0);
            continue;
        };

        manager.index_to_hash.insert(index, place);
    }

    call_original!(arg, arg2)
}

pub fn get_ui_hash_to_stage_hash(stage: &[u8]) -> BTreeMap<Hash40, Hash40> {
    let mut reader = Cursor::new(stage);

    let param_data = prc::read_stream(&mut reader).unwrap();

    let (_, main_list) = &param_data.0[0];

    let ParamKind::List(list) = main_list else {
        unreachable!()
    };

    let mut map = BTreeMap::new();

    for param in list.0.iter() {
        let ParamKind::Struct(param) = param else {
            continue;
        };

        let ui_stage_id = param.0.iter().find_map(|(k, v)| {
            if Hash40(k.0) == Hash40::from("ui_stage_id") {
                let ParamKind::Hash(v) = v else {
                    return None;
                };

                Some(Hash40(v.0))
            } else {
                None
            }
        });

        let stage_place_id = param.0.iter().find_map(|(k, v)| {
            if Hash40(k.0) == Hash40::from("stage_place_id") {
                let ParamKind::Hash(v) = v else {
                    return None;
                };

                Some(Hash40(v.0))
            } else {
                None
            }
        });

        match (ui_stage_id, stage_place_id) {
            (Some(ui), Some(place)) => {
                map.insert(ui, place);
            }
            _ => {}
        }
    }

    map
}

#[skyline::hook(offset = 0x3359d20)]
unsafe fn replace_texture(state: *mut lua::lua_State) -> i32 {
    if lua::lua_isinteger(state, -1) == 1 {
        let index = lua::lua_tointegerx(state, -1, std::ptr::null_mut()) as i32;
        lua::lua_pop(state, 1);

        lua::lua_pushlightuserdata(state, &index as *const i32 as _);
        call_original!(state)
    } else {
        call_original!(state)
    }
}

pub fn install() {
    skyline::install_hooks!(add_to_key_context, is_valid_entrance_param, replace_texture);
}
