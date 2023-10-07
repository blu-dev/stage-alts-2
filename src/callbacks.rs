use hash40::hash40;

extern "C" fn parse_db_files(
    hash: u64,
    buffer: *mut u8,
    buf_len: usize,
    out_size: &mut usize,
) -> bool {
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(buffer, buf_len) };

    let Some(size) = arcropolis_api::load_original_file(hash, &mut buffer) else {
        panic!("failed to load original file");
    };

    *out_size = size;

    if hash == hash40("ui/param/database/ui_bgm_db.prc").0 {
        crate::manager::MANAGER.write().set_bgm_data(buffer);
    } else if hash == hash40("ui/param/database/ui_stage_db.prc").0 {
        crate::manager::MANAGER.write().set_stage_data(buffer);
    }

    true
}

pub fn install() {
    arcropolis_api::register_callback("ui/param/database/ui_stage_db.prc", 1, parse_db_files);
    arcropolis_api::register_callback(
        "ui/param/database/ui_bgm_db.prc",
        10 * 1024 * 1024,
        parse_db_files,
    );
}
