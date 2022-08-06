use anyhow::{Context, Result};
use clap::Parser;
use ctrlc;
use regex::Regex;
use semver;
use serde_derive::Deserialize;
use std::env::temp_dir;
use std::fs;
use toml;

/// Defines the TOML struct that is returned by the Julia loader process.
#[derive(Deserialize, Debug)]
struct Config {
    image: String,
    depot: String,
    load_path: String,
}

/// Julia system image loader.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Version of julia to run.
    #[clap(long, value_parser = validate_version)]
    julia: String,

    /// Name of the system image to use.
    #[clap(long, value_parser = validate_identifier)]
    image: String,

    /// Name of the package that provides the required artifacts.
    #[clap(long, value_parser = validate_identifier)]
    package: String,

    /// Extra arguments to pass to the launched julia process.
    #[clap(value_parser)]
    julia_args: Vec<String>,
}

fn validate_identifier(s: &str) -> Result<String, &'static str> {
    let re = Regex::new(r"^[A-Za-z][A-Za-z0-9_]*$").expect("failed to create regex");
    if re.is_match(s) {
        Ok(String::from(s))
    } else {
        Err("must be a valid Julia ASCII identifier.")
    }
}

fn validate_version(s: &str) -> Result<String, &'static str> {
    let version = semver::Version::parse(s);
    match version {
        Ok(_) => Ok(String::from(s)),
        Err(_) => Err("must be a Julia version number, e.g. 1.7.3"),
    }
}

fn run() -> Result<i32> {
    let args = Args::parse();

    //
    // Step 1:
    //
    // We launch `julia` to lookup the system image path and custom depot required by that image
    // file. `args.package` is expected to be a Julia package installed in the default environment
    // for the version of Julia launched. This package must contain a `config(::Symbol)` function
    // that returns a `(image = "...", depot = "...")` which is captured via stdout in the parent
    // process.
    //

    // Hack to allow for evaluating Julia code from the process started below. For some reason
    // --eval didn't work right.
    let script = temp_dir().join("system-image-loader.jl");
    let path = &script.to_string_lossy();
    fs::write(
        &script,
        format!(
            "import {pkg}; {pkg}.SystemImageLoader.toml({pkg}.config(:{image}))",
            pkg = args.package,
            image = args.image
        ),
    )
    .with_context(|| format!("failed to write temporary Julia script: {}", path))?;

    let julia = std::process::Command::new("julia")
        .arg(format!("+{}", args.julia))
        .args(["--startup-file=no", "--compile=min", "--color=no", path])
        .output()
        .with_context(|| "failed to run julia artifact lookup command.")?;

    //
    // Step 2:
    //
    // We capture the TOML-formatted stdout from the julia-init process and start a second julia
    // process that launches with the provided

    if julia.status.success() {
        let payload = String::from_utf8_lossy(&julia.stdout);
        let config: Config = toml::from_str(&payload)
            .with_context(|| format!("failed to parse TOML payload:\n{}", payload))?;

        // Let the julia process handle ctrl-c. Taken from `juliaup` handling of child procs.
        ctrlc::set_handler(|| ()).with_context(|| "failed to set ctrl-c handler.")?;

        // The main julia process: started with the depot path and sysimage image returned from the
        // first julia init process.
        let mut child_process = std::process::Command::new("julia")
            .args(&args.julia_args)
            .arg(format!("--sysimage={}", config.image))
            .env("JULIA_DEPOT_PATH", config.depot)
            .env("JULIA_LOAD_PATH", config.load_path)
            .spawn()
            .with_context(|| "failed to start main julia process.")?;

        let status = child_process
            .wait()
            .with_context(|| "failed to wait for main julia process to finish.")?;

        let code = match status.code() {
            Some(code) => code,
            None => 1,
        };

        Ok(code)
    } else {
        Err(anyhow::anyhow!(
            "julia init process did not run successfully:\n{}",
            String::from_utf8_lossy(&julia.stderr)
        ))
    }
}

fn main() -> Result<()> {
    // TODO: logging setup.
    let code = run()?;
    std::process::exit(code)
}
