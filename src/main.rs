mod banner;
mod distro;
mod ffi;
mod install;
mod manifest;
mod remove;
mod repo;
mod security;
mod system_check;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        banner::print();
        return ExitCode::SUCCESS;
    }

    match args[1].as_str() {
        "-i" | "--install" => run_install(&args[2..]),
        "-r" | "--remove" => run_remove(&args[2..]),
        "-s" | "--search" => run_search(&args[2..]),
        "-l" | "--list" => run_list(),
        "--info" => run_distro_info(),
        "-h" | "--help" => {
            print_help();
            ExitCode::SUCCESS
        }
        "-v" | "--version" => {
            println!("Katyusha {}", banner::VERSION);
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("[✗] Unknown option: '{other}'");
            eprintln!("    Run 'katyusha --help' to see the available options.");
            ExitCode::FAILURE
        }
    }
}

fn run_install(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        eprintln!("[✗] Missing package name. Usage: sudo katyusha -i <package> [--force]");
        return ExitCode::FAILURE;
    };
    let force = rest.iter().any(|a| a == "--force" || a == "-f");

    match install::install(name, force) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[✗] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_remove(rest: &[String]) -> ExitCode {
    let Some(name) = rest.first() else {
        eprintln!("[✗] Missing package name. Usage: sudo katyusha -r <package>");
        return ExitCode::FAILURE;
    };
    match remove::remove(name) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[✗] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_search(rest: &[String]) -> ExitCode {
    let Some(query) = rest.first() else {
        eprintln!("[✗] Missing search term. Usage: katyusha -s <text>");
        return ExitCode::FAILURE;
    };

    let packages = match repo::load_packages() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[✗] {e}");
            return ExitCode::FAILURE;
        }
    };

    let query_lower = query.to_lowercase();
    let matches: Vec<_> = packages
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&query_lower)
                || p.description
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query_lower)
        })
        .collect();

    if matches.is_empty() {
        println!("No packages found for '{query}'.");
        return ExitCode::SUCCESS;
    }

    for pkg in matches {
        println!(
            "{}  {}{}",
            pkg.name,
            pkg.version,
            pkg.description
                .as_ref()
                .map(|d| format!("  —  {d}"))
                .unwrap_or_default()
        );
    }
    ExitCode::SUCCESS
}

fn run_list() -> ExitCode {
    let installed = manifest::load();
    if installed.is_empty() {
        println!("No packages installed by Katyusha.");
        return ExitCode::SUCCESS;
    }
    for pkg in installed {
        println!("{}  {}", pkg.name, pkg.version);
    }
    ExitCode::SUCCESS
}

fn run_distro_info() -> ExitCode {
    let info = distro::detect();
    println!("Detected distribution: {info}");
    println!("Root: {}", if ffi::is_root() { "yes" } else { "no" });
    ExitCode::SUCCESS
}

fn print_help() {
    banner::print();
    println!();
    println!("Options:");
    println!("  -i, --install <package>          Install a package (requires sudo)");
    println!("      --force / -f                 With -i: install even if already present");
    println!("  -r, --remove  <package>          Remove a package (requires sudo)");
    println!("  -s, --search  <text>             Search the package index");
    println!("  -l, --list                       List installed packages");
    println!("      --info                       Show the detected distro");
    println!("  -h, --help                       Show this help");
    println!("  -v, --version                    Show the version");
    println!();
    println!("Index repository (KATYUSHA_REPO_URL to override):");
    println!("  {}", repo::DEFAULT_INDEX_URL);
}
