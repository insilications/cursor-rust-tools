use crate::project::Project;
use anyhow::Result;
use std::process::Command;

pub fn generate_docs(project: &Project) -> Result<()> {
    // Run cargo doc with custom output directory
    let mut cmd = Command::new("cargo");
    cmd.current_dir(project.root()).args([
        "doc",
        "--workspace",
        "--target-dir",
        project.cache_folder(),
    ]);

    if let Some(extra_env) = project
        .rust_analyzer()
        .and_then(|ra| ra.cargo.as_ref())
        .and_then(|c| c.extra_env.as_ref())
    {
        cmd.envs(extra_env);
        tracing::info!("Added extra_env to cargo doc command");
    }

    let output = cmd.output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to generate documentation"));
    }

    Ok(())
}
