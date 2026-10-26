//! Project tasks. Run with `cargo xtask <task>`.

use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let task = std::env::args().nth(1);
    let rest: Vec<String> = std::env::args().skip(2).collect();
    match task.as_deref() {
        Some("fmt") => fmt(&rest),
        Some("clippy") => clippy(),
        Some("test") => test(),
        Some("layers") => layers(),
        Some("cleanroom") => cleanroom(),
        Some("assets") => assets(),
        Some("wasm") => wasm(),
        Some("perf") => perf(),
        Some("ci") => ci(),
        Some("package") => package(&rest),
        Some(other) => Err(anyhow!("unknown xtask: {other}. See xtask/src/main.rs")),
        None => {
            println!(
                "cargo xtask <task>\n\n\
                 tasks:\n  \
                 fmt       — rustfmt --check (pass `--write` to apply)\n  \
                 clippy    — clippy -D warnings on the workspace\n  \
                 test      — cargo test on the workspace\n  \
                 layers    — enforce the L0->L6 crate dependency order\n  \
                 cleanroom — scan the repo for banned-provenance markers\n  \
                 assets    — verify every tracked asset is listed in ASSETS.md\n  \
                 wasm      — build the engine for wasm32-unknown-unknown (noop if target absent)\n  \
                 perf      — run the perf-budget harness (advisory)\n  \
                 ci        — run everything above\n  \
                 package   — build a redistributable for the host OS\n              \
                 (macOS: .app bundle; Linux: AppDir; Windows: .zip)"
            );
            Ok(())
        }
    }
}

fn repo_root() -> Result<PathBuf> {
    let out = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .context("running cargo locate-project")?;
    if !out.status.success() {
        bail!("cargo locate-project failed");
    }
    let path = String::from_utf8(out.stdout)?.trim().to_string();
    Ok(PathBuf::from(path)
        .parent()
        .ok_or_else(|| anyhow!("workspace Cargo.toml has no parent"))?
        .to_path_buf())
}

fn run(cmd: &mut Command) -> Result<()> {
    let desc = format!("{cmd:?}");
    let status = cmd.status().with_context(|| format!("spawning {desc}"))?;
    if !status.success() {
        bail!("{desc} failed with {status}");
    }
    Ok(())
}

fn fmt(rest: &[String]) -> Result<()> {
    let write = rest.iter().any(|a| a == "--write");
    let mut c = Command::new("cargo");
    c.arg("fmt").arg("--all");
    if !write {
        c.arg("--").arg("--check");
    }
    run(&mut c)
}

fn clippy() -> Result<()> {
    run(Command::new("cargo").args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))
}

fn test() -> Result<()> {
    run(Command::new("cargo").args(["test", "--workspace"]))
}

/// Enforce the layering declared in `docs/architecture.md`. A crate at
/// level N may only depend on crates at levels < N (plus external deps).
fn layers() -> Result<()> {
    use std::collections::HashMap;

    let layers: HashMap<&str, u8> = [
        ("graphcraft_numeric", 1),
        ("graphcraft_stats", 2),
        ("graphcraft_table", 2),
        ("graphcraft_regression", 3),
        ("graphcraft_analysis", 4),
        ("graphcraft_plot", 4),
        ("graphcraft_format", 4),
        ("graphcraft_render", 4),
        ("graphcraft_engine", 5),
        ("graphcraft_ui_egui", 6),
        ("graphcraft", 6),
        ("graphcraft-cli", 6),
        ("graphcraft-bench", 6),
        ("graphcraft-mcp", 6),
        ("graphcraft-web", 6),
    ]
    .into_iter()
    .collect();

    let root = repo_root()?;
    let mut violations = Vec::new();

    for (name, level) in &layers {
        let manifest = manifest_for_crate(&root, name)?;
        let text = std::fs::read_to_string(&manifest)
            .with_context(|| format!("reading {}", manifest.display()))?;
        for (dep, dep_level) in &layers {
            if dep == name {
                continue;
            }
            let needle = format!("{dep} =");
            let needle_dashed = format!(r#"name = "{dep}""#);
            if (text.contains(&needle) || text.contains(&needle_dashed)) && dep_level > level {
                violations.push(format!(
                    "{name} (L{level}) depends on {dep} (L{dep_level}) — must be <= L{level}",
                ));
            }
        }
    }
    if !violations.is_empty() {
        for v in &violations {
            eprintln!("layering violation: {v}");
        }
        bail!("{} layering violation(s)", violations.len());
    }
    println!("layers ok");
    Ok(())
}

fn manifest_for_crate(root: &Path, name: &str) -> Result<PathBuf> {
    // crate dir name differs from package name only for ui-egui.
    let candidates = [
        root.join("crates")
            .join(name.trim_start_matches("graphcraft_"))
            .join("Cargo.toml"),
        root.join("crates")
            .join(name.trim_start_matches("graphcraft_").replace('_', "-"))
            .join("Cargo.toml"),
        root.join("apps").join(name).join("Cargo.toml"),
    ];
    for p in &candidates {
        if p.exists() {
            return Ok(p.clone());
        }
    }
    Err(anyhow!("could not find Cargo.toml for {name}"))
}

/// Scan tracked files for banned-provenance markers. The rule is that
/// nothing in this repository may be derived from GraphPad-authored
/// content; we check for strings that would only appear if the rule had
/// been broken. Allowed references to the trademark are confined to
/// `NOTICE`, `README.md`, `AGENTS.md`, `CLAUDE.md`, `ROADMAP.md`,
/// `docs/`, and the project rulebook itself.
fn cleanroom() -> Result<()> {
    let root = repo_root()?;
    let banned_substrings = [
        "Copyright GraphPad",
        "Copyright (c) GraphPad",
        "(C) GraphPad",
        "GraphPad Software, Inc.",
        "Confidential — GraphPad",
        "Dotmatics Confidential",
    ];
    let allowed_mentions: &[&str] = &[
        "NOTICE",
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        "ROADMAP.md",
        "ASSETS.md",
        "docs/",
        "xtask/",
        "plan/",
    ];
    let mut hits = Vec::new();
    walk(&root, &mut |path| {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if allowed_mentions.iter().any(|m| rel_str.starts_with(m)) {
            return Ok(());
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(());
        };
        for needle in &banned_substrings {
            if text.contains(needle) {
                hits.push(format!("{rel_str}: contains banned marker '{needle}'"));
            }
        }
        Ok(())
    })?;
    if !hits.is_empty() {
        for h in &hits {
            eprintln!("cleanroom: {h}");
        }
        bail!("{} cleanroom violation(s)", hits.len());
    }
    println!("cleanroom ok");
    Ok(())
}

fn walk<F>(root: &Path, f: &mut F) -> Result<()>
where
    F: FnMut(&Path) -> Result<()>,
{
    let skip = ["target", ".git", "plan"];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.iter().any(|s| name == *s) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                f(&p)?;
            }
        }
    }
    Ok(())
}

fn assets() -> Result<()> {
    let root = repo_root()?;
    let inventory = root.join("ASSETS.md");
    let inventory_text = std::fs::read_to_string(&inventory)
        .with_context(|| format!("reading {}", inventory.display()))?;
    let binary_exts = [
        "png", "jpg", "jpeg", "gif", "svg", "ico", "icns", "ttf", "otf", "woff", "woff2", "webp",
        "psd", "ai", "pzfx",
    ];
    let mut missing = Vec::new();
    walk(&root, &mut |path| {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return Ok(());
        };
        if !binary_exts.iter().any(|e| *e == ext.to_lowercase()) {
            return Ok(());
        }
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !inventory_text.contains(&rel_str) {
            missing.push(rel_str);
        }
        Ok(())
    })?;
    if !missing.is_empty() {
        for m in &missing {
            eprintln!("assets: {m} is not listed in ASSETS.md");
        }
        bail!("{} unattributed asset(s)", missing.len());
    }
    println!("assets ok");
    Ok(())
}

fn wasm() -> Result<()> {
    // Check the target is installed. If not, warn but don't fail — the
    // gate becomes hard in CI once the toolchain is set up (ROADMAP M1).
    let out = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();
    let installed = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };
    if !installed.contains("wasm32-unknown-unknown") {
        println!(
            "wasm: wasm32-unknown-unknown target not installed; skipping. \
             Install with `rustup target add wasm32-unknown-unknown`."
        );
        return Ok(());
    }
    run(Command::new("cargo").args([
        "build",
        "-p",
        "graphcraft_engine",
        "--target",
        "wasm32-unknown-unknown",
    ]))
}

fn perf() -> Result<()> {
    // Build release so the numbers mean something.
    run(Command::new("cargo").args(["run", "--release", "-p", "graphcraft-bench", "--"]))
}

fn ci() -> Result<()> {
    fmt(&[])?;
    clippy()?;
    test()?;
    layers()?;
    cleanroom()?;
    assets()?;
    wasm()?;
    println!("ci ok");
    Ok(())
}

/// Build a redistributable package for the host OS. The native
/// desktop binary (`graphcraft`) is release-built first; the
/// output is dropped under `target/package/`.
///
/// Arguments: none. Future expansion may take `--target` to
/// cross-package, but for now the host OS decides.
fn package(_args: &[String]) -> Result<()> {
    let root = repo_root()?;
    let pkg_dir = root.join("target/package");
    std::fs::create_dir_all(&pkg_dir).with_context(|| format!("mkdir -p {}", pkg_dir.display()))?;

    println!("cargo build --release -p graphcraft");
    run(Command::new("cargo")
        .current_dir(&root)
        .args(["build", "--release", "-p", "graphcraft"]))?;

    let version = env!("CARGO_PKG_VERSION");
    if cfg!(target_os = "macos") {
        package_macos(&root, &pkg_dir, version)
    } else if cfg!(target_os = "linux") {
        package_linux(&root, &pkg_dir, version)
    } else if cfg!(target_os = "windows") {
        package_windows(&root, &pkg_dir, version)
    } else {
        bail!("packaging on this host OS is not supported yet")
    }
}

fn package_macos(root: &Path, pkg_dir: &Path, version: &str) -> Result<()> {
    let bin = root.join("target/release/graphcraft");
    if !bin.exists() {
        bail!("expected binary not found: {}", bin.display());
    }
    let app = pkg_dir.join("GraphCraft.app");
    let _ = std::fs::remove_dir_all(&app);
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    std::fs::create_dir_all(&macos)?;
    std::fs::create_dir_all(&resources)?;

    std::fs::copy(&bin, macos.join("graphcraft"))
        .with_context(|| format!("copy binary into {}", macos.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(macos.join("graphcraft"))?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(macos.join("graphcraft"), p)?;
    }

    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n  \
           <key>CFBundleName</key><string>GraphCraft</string>\n  \
           <key>CFBundleDisplayName</key><string>GraphCraft</string>\n  \
           <key>CFBundleIdentifier</key><string>com.graphcraft.app</string>\n  \
           <key>CFBundleVersion</key><string>{version}</string>\n  \
           <key>CFBundleShortVersionString</key><string>{version}</string>\n  \
           <key>CFBundlePackageType</key><string>APPL</string>\n  \
           <key>CFBundleExecutable</key><string>graphcraft</string>\n  \
           <key>LSMinimumSystemVersion</key><string>10.13</string>\n  \
           <key>NSHighResolutionCapable</key><true/>\n  \
           <key>NSHumanReadableCopyright</key><string>MIT OR Apache-2.0</string>\n\
         </dict>\n</plist>\n"
    );
    std::fs::write(contents.join("Info.plist"), plist)?;
    println!("packaged {}", app.display());
    Ok(())
}

fn package_linux(root: &Path, pkg_dir: &Path, version: &str) -> Result<()> {
    let bin = root.join("target/release/graphcraft");
    if !bin.exists() {
        bail!("expected binary not found: {}", bin.display());
    }
    let appdir = pkg_dir.join("GraphCraft.AppDir");
    let _ = std::fs::remove_dir_all(&appdir);
    let bin_dir = appdir.join("usr/bin");
    std::fs::create_dir_all(&bin_dir)?;
    std::fs::copy(&bin, bin_dir.join("graphcraft"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(bin_dir.join("graphcraft"))?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(bin_dir.join("graphcraft"), p)?;
    }
    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=GraphCraft\n\
         Comment=Scientific graphing and analysis (v{version})\n\
         Exec=graphcraft\n\
         Terminal=false\n\
         Categories=Science;Education;\n"
    );
    std::fs::write(appdir.join("graphcraft.desktop"), desktop)?;
    let run = "#!/bin/sh\n\
               HERE=\"$(dirname \"$(readlink -f \"${0}\")\")\"\n\
               exec \"${HERE}/usr/bin/graphcraft\" \"$@\"\n";
    std::fs::write(appdir.join("AppRun"), run)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(appdir.join("AppRun"))?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(appdir.join("AppRun"), p)?;
    }
    println!("packaged {}", appdir.display());
    println!(
        "  → turn into an AppImage with: appimagetool {}",
        appdir.display()
    );
    Ok(())
}

fn package_windows(root: &Path, pkg_dir: &Path, version: &str) -> Result<()> {
    let bin = root.join("target/release/graphcraft.exe");
    if !bin.exists() {
        bail!("expected binary not found: {}", bin.display());
    }
    let stage = pkg_dir.join(format!("GraphCraft-{version}"));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    std::fs::copy(&bin, stage.join("graphcraft.exe"))?;
    let readme = format!(
        "GraphCraft v{version}\n\
         ====================\n\
         Run graphcraft.exe to launch the app.\n\
         Licensed under MIT OR Apache-2.0.\n"
    );
    std::fs::write(stage.join("README.txt"), readme)?;
    println!("packaged {}", stage.display());
    println!(
        "  → zip into a release with: Compress-Archive {} GraphCraft-{version}.zip",
        stage.display()
    );
    Ok(())
}
