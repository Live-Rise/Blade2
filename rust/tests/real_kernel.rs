use std::path::{Path, PathBuf};

use blade2_rs::kernel::{Kernel, Launch};

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
