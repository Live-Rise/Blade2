pub mod contextmeter;
pub mod crashlog;
pub mod i18n;
pub mod keys;
pub mod kernel;
pub mod layout;
// #82 壳落盘四类文件的纯函数层（渲染/解析/数据家路径）。就近 layout：两者共用同一条
// `LOCALAPPDATA → data_home_root → DATA_HOME_DIR` 链。接线在 main.rs，本模块不含任何 UI 调用点。
pub mod shellfiles;
// #138 右栏 dock 的纯状态机（标签/分栏/全屏/两枚格 + 离线单测）。与 layout 同族：
// 只算不画 —— 渲染与接线在 main.rs 的 K2–K4 那一刀，本模块零渲染依赖。
pub mod dock;
// #76 工作区文件右栏（Files 面板）那六发 workspaceFiles/* 的纯层：回执解析、目录树/条目模型、
// 预览截断判据、base64 校验与 scope-id 态机。与 dock/shellfiles 同族：只算不画 ——
// 六发派发在 main.rs（MR2），args ctor 在 kernel.rs（KW3），出图与流协议另立一族。
pub mod filespanel;
pub mod mux;
pub mod procguard;
pub mod scroll;
pub mod theme;
pub mod tokens;
pub mod turnrail;
pub mod subagents;
pub mod capabilities;
// #160 B 刀第二片（NP1）：原生文件选择器**原语** —— comdlg32!GetOpenFileNameW 裸 FFI、零新依赖。
// 与 updatecheck 同族：只交货架不接线（线程模型裁定与「非测试读者 = 0」的备案都在模块头）。
pub mod nativepick;
// #160 B 刀第一片（UC-K1）：更新检查那条腿的纯层 —— 四段版本比较、releases/latest 回执解析、WinHTTP
// 请求描述子 + 执行腿。与 dock/filespanel 同族：只算不画 —— 接线与 toast 都在 main.rs 那一刀，本模块零 UI 调用点。
pub mod updatecheck;
// 托盘底座**原语**（TR1 第 1 把刀）：shell32!Shell_NotifyIconW 的 NIM_ADD / NIM_DELETE 两档 + hIcon 解析
// （运行时 Assets/app.ico）。与 toast.rs（只发 NIM_MODIFY 的气球腿）各带一份结构体 = 设计上的两份真相，
// 不许合并；子类化回调臂 / 原生菜单 / main.rs 接线一律归下一批（范围与备案见模块头）。
pub mod tray;
