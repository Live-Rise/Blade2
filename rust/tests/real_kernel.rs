use std::path::{Path, PathBuf};

use blade2_rs::kernel::{Kernel, Launch, is_bundled};

fn repo_root(manifest: &Path) -> PathBuf {
    manifest
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.to_path_buf())
}

/// 真内核端到端：手动跑 `cargo test -- --ignored -- --nocapture`。
/// DSH_HOME 与 cwd 都隔离到 rust/target 下，不写主干运行数据；Kernel/ 只读执行。
#[test]
#[ignore = "会启动真实 Kernel/node.exe，需手动执行"]
fn real_kernel_handshake_and_session_list() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kernel_dir = repo_root(&manifest).join("Kernel");
    let home = manifest.join("target/dsh-home");
    let cwd = manifest.join("target/kernel-cwd");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    let bin_js = kernel_dir.join("dsh").join("lib").join("bin.js");
    assert!(
        kernel_dir.join("node.exe").is_file(),
        "缺少 {}",
        kernel_dir.join("node.exe").display()
    );
    assert!(bin_js.is_file(), "缺少 {}", bin_js.display());
    // #84-A：这里手搓的 argv 就是两级 launcher 的**第 1 级（内置）**的形状 —— `node.exe` +
    // `bin.js` + `web --no-open --port 0`，走正常 argv 列表（内置级 std 的转义已等价于主干那对
    // 外层双引号，见 `launcher_argv`），不是回退级那颗必须原样交给 cmd.exe 的嵌套引号串
    // ⇒ `args_verbatim = false`。
    // `is_bundled` 的口径（§1.7-1）是「内置那两颗 `File.Exists` 的与」，与本次实际起了谁无关，
    // 故跟着上面那两条前提**现算**而非硬编码：两文件都在 ⇒ true，与 `Launch::from_env` 同结果。
    let (node_probe, bin_js_probe) = (
        kernel_dir.join("node.exe").is_file().then_some(kernel_dir.join("node.exe")),
        bin_js.is_file().then_some(bin_js.clone()),
    );
    let session_cwd = cwd.display().to_string();
    let launch = Launch {
        exe: kernel_dir.join("node.exe"),
        args: vec![
            bin_js.display().to_string(),
            "web".into(),
            "--no-open".into(),
            "--port".into(),
            "0".into(),
        ],
        args_verbatim: false,
        is_bundled: is_bundled(&node_probe, &bin_js_probe),
        dsh_home: Some(home),
        path_prepend: Some(kernel_dir.join("bin")),
        working_dir: Some(cwd),
    };

    let mut kernel = Kernel::start(&launch).expect("真实内核握手失败");
    println!("url = {}", kernel.url);
    println!("pid = {:?}", kernel.pid());
    println!("handshake lines = {}", kernel.log.len());
    let rows = kernel.list_sessions().expect("真实内核 session/list 失败");
    println!("sessions = {}", rows.len());
    for row in rows.iter().take(5) {
        println!("  {} | {} | {}", row.id, row.title, row.cwd);
    }

    let created = kernel
        .create_session(&session_cwd)
        .expect("真实内核 session/create 失败");
    println!("created = {created}");
    let after = kernel.list_sessions().expect("创建后重新 list 失败");
    assert!(
        after.iter().any(|row| row.id == created),
        "新会话 {created} 应出现在列表中"
    );
    kernel.shutdown();
}
