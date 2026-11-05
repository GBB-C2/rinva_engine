#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod graph;
#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod i18n;

#[cfg(windows)]
#[path = "platform/windows/mod.rs"]
mod platform;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--validate") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage: rinva --validate path/to/story.rinva");
            std::process::exit(2);
        };
        let path = std::path::Path::new(path);
        match rinva_core::storage::read_project(path) {
            Ok(project) => {
                let root = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(std::path::Path::new("."));
                let issues = rinva_core::inspect_project(&project, root);
                for issue in &issues {
                    eprintln!(
                        "{:?} [{}]: {}",
                        issue.severity,
                        issue.scene_id.as_deref().unwrap_or("project"),
                        issue.message
                    );
                }
                if issues
                    .iter()
                    .any(|i| i.severity == rinva_core::Severity::Error)
                {
                    std::process::exit(1);
                }
                println!("Project is valid ({} warnings)", issues.len());
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(windows)]
    let smoke_output = args
        .first()
        .is_some_and(|arg| arg == "--smoke-test")
        .then(|| {
            args.get(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "smoke-output".into())
        });
    #[cfg(windows)]
    if let Err(error) = platform::run(args) {
        if let Some(output) = smoke_output {
            let _ = std::fs::create_dir_all(&output);
            let _ = std::fs::write(output.join("native-error.txt"), &error);
        } else {
            platform::show_fatal(&error);
        }
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("Rinva Editor 0.1 targets Windows 10/11. The core and --validate command work on this platform.");
        std::process::exit(2);
    }
}
