use crate::project::Project;
use anyhow::Result;
use std::process::Command;
use std::process::Stdio;

pub fn generate_docs(project: &Project) -> Result<()> {
    // Run cargo doc with custom output directory
    let mut cmd = Command::new("cargo");

    // core args – push directly to avoid Vec allocation
    cmd.args([
        "doc",
        "--quiet",
        "--workspace",
        "--target-dir",
        project.cache_folder(),
    ]);

    if let Some(ra) = project.rust_analyzer()
        && let Some(cargo_cfg) = ra.cargo.as_ref()
    {
        if let Some(target) = cargo_cfg.target.as_ref() {
            tracing::info!("generate_docs - target: {target}");
            cmd.args(["--target", target]);
        }

        if let Some(extra_env) = cargo_cfg.extra_env.as_ref() {
            cmd.envs(extra_env);
            tracing::info!("Added extra_env to cargo doc command");
        }
    }
    // if let Some(ra) = project.rust_analyzer() {
    //     if let Some(cargo_cfg) = ra.cargo.as_ref() {
    //         if let Some(target) = cargo_cfg.target.as_ref() {
    //             tracing::info!("CargoRemote::check - target: {target}");
    //             cmd.args(["--target", target]);
    //         }

    //         if let Some(extra_env) = cargo_cfg.extra_env.as_ref() {
    //             cmd.envs(extra_env);
    //             tracing::info!("Added extra_env to cargo doc command");
    //         }
    //     }
    // }

    cmd.current_dir(project.root())
        .stdin(Stdio::null())
        .stdout(Stdio::null());

    let status = cmd.status()?; // avoids buffering stdout/stderr

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to generate documentation"));
    }
    Ok(())
}
