//! Wallpaper Engine 工坊**壳侧直连**：定位工坊目录、列壁纸、套用进皮肤槽位。
//!
//! # 为什么是壳直连（2026-11-05 乱弹批三件之三）
//!
//! 主干那半边依赖内核插件 `@baiiii/dsh-wallpaper-local` 的服务端路由
//! （`GET api/wallpaper/list` / `/full` / `/video`，`DshWallpaperClient`）——插件由
//! 主干引导器默认安装，而分叉的官方内核树（0.2.1-rc.3，零补丁）里**没有这个包**，
//! `node_modules/@deepseek-ai` 与 `lib/*.js` 全文零命中 ⇒ 分叉的 WE 二级页永远停在
//! `WallpaperRouteStatus.Missing` 那档文案（「未检测到壁纸插件」），就是用户报的
//! 「无法正确载入 Wallpaper Engine」。铺 HTTP 通路 + 自装家族包两条路都撞「零补丁 /
//! 最小形态」的墙；而主干**自己**对视频项预览图就是壳直读工坊目录（`WeWorkspace.cs`
//! 文件头写明缘由），工坊本来就在本机磁盘上 ⇒ 壳直连 = 拿主干已有的第三条路走完，
//! 不碰内核、不引 HTTP。
//!
//! # 与主干套用语义的逐条对位（`MainWindow.Wallpaper.cs:222-282`）
//!
//! * 套用 = **下载进槽位再走既有管线**；分叉无 HTTP ⇒ 下载腿换 `fs::copy`（本机盘内拷贝）。
//! * 视频 → `shell-skin.video`、图片 → `shell-skin.img`；进视频槽**先停泵**（调用侧，
//!   `Msg::WeApply` 臂抄 `Msg::SkinVideoImportPick` 的顺序红线）。
//! * 单槽互斥：进视频槽删图片槽、反之亦然（主干 `DeleteSkinQuietly` 同款，删不掉不回滚）。
//! * `shell-skin.we` 记「当前背景来自哪个 WE 壁纸」（[`shellfiles::save_skin_we_id`]，
//!   裸文本恒等写入）；本地导入会清它（那两发 `clear_skin_we_id` 既有调用）。
//! * kind 判据：主干插件吐 `kind="video"`；分叉直读 `project.json` ⇒ 按 **file 扩展名**
//!   折算（mp4/webm/wmv = 视频，png/jpg/jpeg/bmp/gif = 图片），其余（scene/web/
//!   application 的 pkg/html）跳过 —— 主干「只把这些路由当又一个皮肤素材源」的口径，
//!   scene 壁纸本来就进不了静态/视频槽。
//!
//! # 工坊定位（`WeWorkspace.cs:88-119` 同源）
//!
//! Steam 根 = 注册表 `HKCU\Software\Valve\Steam` 的 `SteamPath`（正斜杠盘符路径，替换）
//! 与 `SteamExe` 的目录，外加 `D:\Steam` 兜底（主干插件内置同款）；每个根再吃
//! `steamapps\libraryfolders.vdf` 的 `"path"` 库清单；第一个存在
//! `steamapps\workshop\content\431960` 的库即工坊根。

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::shellfiles;
use crate::skinvideo;

/// Wallpaper Engine 的 Steam AppId（主干 `WeWorkspace.cs:22` 同款）。
const WE_APP_ID: &str = "431960";
/// Steam 的 HKCU 键（主干 `:23`）。
const STEAM_KEY: &str = "Software\\Valve\\Steam";
/// 主库兜底（主干 `SteamRoots` 里插件内置那一条，`:117`）。
const STEAM_FALLBACK: &str = "D:\\Steam";
/// 视频/图片素材的扩展名白名单（小写比对）。其余 `project.json` 里的 file 一律跳过。
const VIDEO_EXTS: [&str; 3] = ["mp4", "webm", "wmv"];
const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "bmp", "gif"];
/// 预览图的候选名（主干 `PreviewExts` 同序，`:25`）。
const PREVIEW_EXTS: [&str; 6] = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// 一个可套用的 WE 壁纸。`file` / `preview` 都是工坊目录内的**绝对路径**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeItem {
    /// 工坊 item 目录名（workshop id 数字串）。`shell-skin.we` 里记的就是它。
    pub id: String,
    /// `project.json` 的 `title`（插件清单的 `name` 同源）。
    pub name: String,
    /// `file` 是视频（进 `shell-skin.video` 槽）。
    pub is_video: bool,
    /// 素材文件绝对路径。
    pub file: PathBuf,
    /// 预览图绝对路径（视频项 = 工坊 `preview.*`；图片项 = 素材自身）。缺预览图 = `None`。
    pub preview: Option<PathBuf>,
}

/// `RegGetValueW` 的 `RRF_RT_REG_SZ`（winreg.h 常量；theme.rs 同款声明形态）。
const RRF_RT_REG_SZ: u32 = 0x0000_0002;
/// `HKEY_CURRENT_USER`（winreg.h 预定义句柄）。
const HKEY_CURRENT_USER: isize = 0x8000_0001_isize;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        hkey: isize,
        lp_sub_key: *const u16,
        lp_value: *const u16,
        dw_flags: u32,
        pdw_type: *mut u32,
        pv_data: *mut core::ffi::c_void,
        pcb_data: *mut u32,
    ) -> i32;
}

/// 读一条 REG_SZ 字符串值；键/值不存在或类型不符 = `None`（主干 `ReadRegString` 同档）。
fn read_reg_string(sub_key: &str, value: &str) -> Option<String> {
    // RegGetValueW 要 UTF-16 + NUL；两条键名都是 ASCII，栈上缓冲一次填够。
    let mut sub_key_buf = [0u16; 80];
    let mut value_buf = [0u16; 64];
    let fill = |buffer: &mut [u16], source: &str| {
        let count = source.encode_utf16().count();
        if count >= buffer.len() {
            return false;
        }
        for (slot, unit) in buffer.iter_mut().zip(source.encode_utf16()) {
            *slot = unit;
        }
        true
    };
    if !fill(&mut sub_key_buf, sub_key) || !fill(&mut value_buf, value) {
        return None;
    }
    let mut size = 0u32; // 先问长度：第一发传空缓冲拿字节数
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub_key_buf.as_ptr(),
            value_buf.as_ptr(),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut size,
        )
    } == 0
        && size >= 2;
    if !ok {
        return None;
    }
    let mut buffer = vec![0u8; size as usize];
    let mut got = size;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub_key_buf.as_ptr(),
            value_buf.as_ptr(),
            RRF_RT_REG_SZ,
            core::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut got,
        )
    } == 0;
    if !ok || got < 2 {
        return None;
    }
    // 末尾 NUL 截掉再折 UTF-8；非偶数字节（坏注册表）按可折长度截。
    let units: Vec<u16> = buffer[..(got as usize / 2) * 2]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16(&units).ok().filter(|text| !text.is_empty())
}

/// `libraryfolders.vdf` 里每个库的 `"path"` 值（主干 `ReadLibraryFolders` 的手写扫描版：
/// 分叉不引 regex crate，VDF 就这一种取值 ⇒ 逐字符找 `"path"` 后第一对引号串）。
/// 值里的 `\\` 转义折成单反斜杠；读不动的文件 = 空表。
fn read_library_folders(vdf: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(vdf) else {
        return Vec::new();
    };
    let mut libraries = Vec::new();
    let mut rest = text.as_str();
    while let Some(pos) = rest.find("\"path\"") {
        rest = &rest[pos + "\"path\"".len()..];
        // 跳过键名与值之间的空白，吃下一对引号里的内容
        let Some(open) = rest.find('"') else { break };
        let Some(len) = rest[open + 1..].find('"') else { break };
        let raw = &rest[open + 1..open + 1 + len];
        rest = &rest[open + 1 + len..];
        if raw.is_empty() {
            continue;
        }
        libraries.push(PathBuf::from(raw.replace("\\\\", "\\")));
    }
    libraries
}

/// Steam 库根清单：注册表两条 + 兜底一条，各自再并 libraryfolders.vdf 的库。
/// 去重保序（主干 `SteamLibraries` 的 `seen` 同款）。全空 = 本机没装 Steam。
fn steam_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut push = |path: PathBuf, roots: &mut Vec<PathBuf>| {
        if !path.as_os_str().is_empty() && !roots.contains(&path) {
            roots.push(path);
        }
    };
    // `SteamPath` 是正斜杠盘符路径（`D:/Steam`），主干 `Replace('/', '\\')` 同款折回。
    if let Some(path) = read_reg_string(STEAM_KEY, "SteamPath") {
        push(PathBuf::from(path.replace('/', "\\")), &mut roots);
    }
    if let Some(exe) = read_reg_string(STEAM_KEY, "SteamExe") {
        if let Some(dir) = Path::new(&exe).parent() {
            push(dir.to_path_buf(), &mut roots);
        }
    }
    push(PathBuf::from(STEAM_FALLBACK), &mut roots);
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in &roots {
        for library in read_library_folders(&root.join("steamapps").join("libraryfolders.vdf")) {
            if !libraries.contains(&library) {
                libraries.push(library);
            }
        }
    }
    for library in libraries {
        push(library, &mut roots);
    }
    roots
}

/// 工坊根（`steamapps\workshop\content\431960`）：第一个有它的库；没有 = `None`
/// （本机没装 WE，调用侧落「未检测到」那档文案）。
#[must_use]
pub fn probe_root() -> Option<PathBuf> {
    for library in steam_roots() {
        let candidate = library.join("steamapps").join("workshop").join("content").join(WE_APP_ID);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// 扩展名（小写）比对；无扩展名 = 空串。
fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 工坊 item 目录的 `project.json` 里这三格；缺 `title`/`file` = `None`（跳过该目录）。
struct ProjectEntry {
    title: String,
    file: String,
}

fn parse_project_json(dir: &Path) -> Option<ProjectEntry> {
    let text = std::fs::read_to_string(dir.join("project.json")).ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    let title = json.get("title")?.as_str()?.to_string();
    let file = json.get("file")?.as_str()?.to_string();
    if title.is_empty() || file.is_empty() {
        return None;
    }
    Some(ProjectEntry { title, file })
}

/// 预览图：`project.json` 的 `preview` 字段优先（相对 item 目录），缺了按 `preview.*`
/// 逐扩展名试（主干 `PreviewFileFor` 同序）；都拿不到 = `None`。
fn preview_for(dir: &Path, declared: Option<&str>) -> Option<PathBuf> {
    if let Some(name) = declared {
        if !name.is_empty() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for ext in PREVIEW_EXTS {
        let candidate = dir.join(format!("preview.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 列工坊壁纸（`root` 的每个子目录读 `project.json`）。素材类型不在两槽白名单里的
/// （scene/web/application 的 pkg/html/exe）跳过；目录名即 id。**无序化处理**：
/// 按目录名数字序排（`3` < `10`），字符串序会把 `10` 排到 `3` 前面。
#[must_use]
pub fn list(root: &Path) -> Vec<WeItem> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort_by_key(|dir| {
        dir.file_name()
            .map(|name| name.to_string_lossy().parse::<u64>().unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX)
    });
    let mut items = Vec::new();
    for dir in dirs {
        let Some(id) = dir.file_name().map(|name| name.to_string_lossy().to_string()) else {
            continue;
        };
        let Some(entry) = parse_project_json(&dir) else {
            continue;
        };
        let file = dir.join(&entry.file);
        if !file.is_file() {
            continue;
        }
        let ext = ext_of(&file);
        let (is_video, allowed) = (
            VIDEO_EXTS.contains(&ext.as_str()),
            VIDEO_EXTS.contains(&ext.as_str()) || IMAGE_EXTS.contains(&ext.as_str()),
        );
        if !allowed {
            continue;
        }
        let preview = if is_video {
            preview_for(&dir, None)
        } else {
            // 图片项的素材本身就是最好的缩略图；preview.* 有则更轻（工坊预览图通常小）。
            preview_for(&dir, None).or_else(|| Some(file.clone()))
        };
        items.push(WeItem {
            id,
            name: entry.title,
            is_video,
            file,
            preview,
        });
    }
    items
}

/// 套用的落点对：`(素材源, 视频槽?, 对称槽名)`。槽名 = [`skinvideo`] 的两枚
/// （`shellfiles` 白名单不收素材文件，见那边模块头）。
fn slot_plan(item: &WeItem) -> (PathBuf, &'static str, &'static str) {
    if item.is_video {
        (item.file.clone(), skinvideo::SKIN_VIDEO_FILE, skinvideo::SKIN_IMG_FILE)
    } else {
        (item.file.clone(), skinvideo::SKIN_IMG_FILE, skinvideo::SKIN_VIDEO_FILE)
    }
}

/// 套用 = **拷进槽位 + 对称槽让位 + 写 WE 标记**（主干 `ApplyWallpaperAsync` 的盘内版）。
/// 拷失败就抛（调用侧弹状态行，不回滚——主干 `File.Copy` 抛了同档）。
///
/// # Errors
/// 源读不动 / 目标写不动（目标被占用由**调用侧先停泵**保证——MF reader 持着
/// `shell-skin.video` 时覆盖会撞共享冲突）。
pub fn apply(item: &WeItem, data_home: &Path) -> io::Result<()> {
    let (source, slot, opposite) = slot_plan(item);
    let dest = data_home.join(slot);
    std::fs::copy(&source, &dest)?;
    // 单槽互斥（主干 `DeleteSkinQuietly`：删不掉不碍事，不回滚）。WE 标记同档：
    // 「纯展示，删了也不影响背景本身」（主干 `ShellWeFile` 注释原文）⇒ 写失败不碍套用。
    let _ = std::fs::remove_file(data_home.join(opposite));
    let _ = shellfiles::save_skin_we_id_to(&item.id, &data_home.join(shellfiles::SKIN_WE_FILE));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 假工坊：一个根 + 三个 item（视频 / 图片 / 无 preview 的视频）+ 一个坏目录。
    fn fake_root(base: &Path) -> PathBuf {
        let root = base.join("workshop").join("content").join(WE_APP_ID);
        // 视频项：全字段齐
        let video = root.join("2890143");
        fs::create_dir_all(&video).unwrap();
        fs::write(video.join("project.json"), r#"{"title":"鸣潮 4K","type":"video","file":"content.mp4"}"#).unwrap();
        fs::write(video.join("content.mp4"), b"fake mp4").unwrap();
        fs::write(video.join("preview.jpg"), b"fake jpg").unwrap();
        // 图片项：无 preview 字段行 ⇒ 素材自身当缩略图
        let image = root.join("3002");
        fs::create_dir_all(&image).unwrap();
        fs::write(image.join("project.json"), r#"{"title":"雪山","type":"video","file":"wallpaper.jpg"}"#).unwrap();
        fs::write(image.join("wallpaper.jpg"), b"fake image").unwrap();
        // scene 壁纸（pkg）：不在两槽白名单 ⇒ 跳过
        let scene = root.join("3100");
        fs::create_dir_all(&scene).unwrap();
        fs::write(scene.join("project.json"), r#"{"title":"动态场景","type":"scene","file":"scene.pkg"}"#).unwrap();
        fs::write(scene.join("scene.pkg"), b"fake pkg").unwrap();
        // 缺 project.json 的目录：跳过
        fs::create_dir_all(root.join("3200")).unwrap();
        root
    }

    /// 工坊 id 按数字序排（`3` 在 `10` 前），字符串序会把 10 排到 3 前面。
    #[test]
    fn list_walks_items_in_numeric_order_and_filters_the_two_slots() {
        let base = std::env::temp_dir().join(format!("blade2-we-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = fake_root(&base);
        // 数字序用例：再补一个更小 id 的目录
        let extra = root.join("2890142");
        fs::create_dir_all(&extra).unwrap();
        fs::write(extra.join("project.json"), r#"{"title":"早的","type":"video","file":"a.mp4"}"#).unwrap();
        fs::write(extra.join("a.mp4"), b"x").unwrap();
        fs::write(extra.join("preview.png"), b"p").unwrap();

        let items = list(&root);
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["3002", "2890142", "2890143"], "数字序（3002 < 2890142）+ scene/空目录被滤");
        let video = &items[2];
        assert!(video.is_video);
        assert_eq!(video.name, "鸣潮 4K");
        assert_eq!(
            video.preview.as_deref(),
            Some(video.file.parent().unwrap().join("preview.jpg").as_path()),
            "视频项缩略图 = 工坊 preview.*"
        );
        let image = &items[0];
        assert!(!image.is_video);
        assert_eq!(image.preview, Some(image.file.clone()), "图片项无 preview 行 ⇒ 素材自身当缩略图");
        let _ = fs::remove_dir_all(&base);
    }

    /// VDF 路径行的 `\\` 转义折成单反斜杠；缺文件 = 空表。
    #[test]
    fn library_folders_unescape_and_tolerate_a_missing_vdf() {
        let base = std::env::temp_dir().join(format!("blade2-we-vdf-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let vdf = base.join("libraryfolders.vdf");
        fs::write(
            &vdf,
            "\"libraries\"\n{\n  \"0\"\n  {\n    \"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n  }\n  \"1\"\n  {\n    \"path\"\t\t\"D:\\\\SteamLibrary\"\n  }\n}\n",
        )
        .unwrap();
        let libraries = read_library_folders(&vdf);
        assert_eq!(
            libraries,
            vec![
                PathBuf::from("C:\\Program Files (x86)\\Steam"),
                PathBuf::from("D:\\SteamLibrary"),
            ]
        );
        assert!(read_library_folders(&base.join("absent.vdf")).is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    /// 套用：视频进视频槽 + 图片槽让位 + `shell-skin.we` 写 id；图片套用反之。
    #[test]
    fn apply_copies_into_the_slot_with_the_opposite_yielding() {
        let base = std::env::temp_dir().join(format!("blade2-we-apply-{}", std::process::id()));
        let root = fake_root(&base);
        let home = base.join("home");
        fs::create_dir_all(&home).unwrap();
        let items = list(&root);
        let video = items.iter().find(|item| item.is_video).unwrap();

        apply(video, &home).unwrap();
        assert!(home.join(skinvideo::SKIN_VIDEO_FILE).is_file());
        assert!(!home.join(skinvideo::SKIN_IMG_FILE).exists());
        assert_eq!(
            shellfiles::load_skin_we_id_from(&home.join(shellfiles::SKIN_WE_FILE)).as_deref(),
            Some("2890143"),
        );

        let image = items.iter().find(|item| !item.is_video).unwrap();
        apply(image, &home).unwrap();
        assert!(home.join(skinvideo::SKIN_IMG_FILE).is_file());
        assert!(!home.join(skinvideo::SKIN_VIDEO_FILE).exists(), "图片套用 ⇒ 视频槽让位");
        assert_eq!(
            shellfiles::load_skin_we_id_from(&home.join(shellfiles::SKIN_WE_FILE)).as_deref(),
            Some("3002"),
        );
        let _ = fs::remove_dir_all(&base);
    }
}
