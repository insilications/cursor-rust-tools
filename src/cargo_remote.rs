use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json as json;
use tokio::process::Command;

use crate::project::Project;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum CargoMessage {
    CompilerArtifact(json::Value),
    BuildScriptExecuted(json::Value),
    CompilerMessage { message: CompilerMessage },
    BuildFinished { success: bool },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CompilerMessage {
    pub rendered: String,
    pub code: Option<json::Value>,
    pub level: String,
    pub spans: Vec<CompilerMessageSpan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CompilerMessageSpan {
    pub column_start: usize,
    pub column_end: usize,
    pub file_name: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Clone, Debug)]
pub struct CargoRemote {
    repository: Project,
    command: Option<String>,
    target: Option<String>,
    extra_env: Option<HashMap<String, String>>,
}

impl CargoRemote {
    pub fn new(repository: Project) -> Self {
        #[allow(clippy::option_if_let_else)]
        let (command, target, extra_env) = match repository.rust_analyzer() {
            Some(ra) => {
                let command = ra.check.as_ref().and_then(|c| c.command.clone());
                match &ra.cargo {
                    Some(cargo) => (command, cargo.target.clone(), cargo.extra_env.clone()),
                    None => (command, None, None),
                }
            }
            None => (None, None, None),
        };

        Self {
            repository,
            command,
            target,
            extra_env,
        }
    }

    async fn run_cargo_command(
        &self,
        args: &[&str],
        backtrace: bool,
    ) -> Result<(Vec<CargoMessage>, Vec<String>)> {
        // Build the command first
        let mut cmd = Command::new("cargo");
        cmd.current_dir(self.repository.root()).args(args);

        if let Some(extra_env) = self.extra_env.as_ref().filter(|m| !m.is_empty()) {
            cmd.envs(extra_env);
        }
        cmd.env("RUST_BACKTRACE", if backtrace { "full" } else { "0" });

        let output = cmd.output().await?;
        // Re-use the buffer returned by `Command` to avoid an extra allocation
        let stdout_str = std::str::from_utf8(&output.stdout)?;

        let mut messages = Vec::new();
        let mut test_messages = Vec::new();
        for line in stdout_str.lines().filter(|line| !line.is_empty()) {
            match json::from_str::<CargoMessage>(line) {
                Ok(msg) => {
                    messages.push(msg);
                }
                Err(_) => {
                    // Cargo test doesn't respect `message-format=json`
                    test_messages.push(line.to_owned());
                }
            }
        }

        Ok((messages, test_messages))
    }

    pub async fn check(&self, only_errors: bool) -> Result<Vec<String>> {
        let cmd = self.command.as_deref().unwrap_or("check");
        tracing::info!("cmd: {cmd}");

        let (messages, _) = match self.target.as_deref() {
            Some(target) => {
                tracing::info!("target: {target}");
                self.run_cargo_command(
                    &[cmd, "--quiet", "--message-format=json", "--target", target],
                    false,
                )
                .await?
            }
            None => {
                self.run_cargo_command(&[cmd, "--quiet", "--message-format=json"], false)
                    .await?
            }
        };

        Ok(messages
            .into_iter()
            .filter_map(|message| match message {
                CargoMessage::CompilerMessage { message } => {
                    if only_errors && message.level != "error" {
                        None
                    } else {
                        Some(message.rendered)
                    }
                }
                _ => None,
            })
            .collect::<Vec<_>>())
    }

    pub async fn test(&self, test_name: Option<String>, backtrace: bool) -> Result<Vec<String>> {
        let mut args = vec!["test", "--message-format=json"];
        if let Some(t) = self.target.as_deref() {
            args.push(t);
        }
        if let Some(ref test_name) = test_name {
            args.push("--");
            args.push("--nocapture");
            args.push(test_name);
        }
        let (_, messages) = self.run_cargo_command(&args, backtrace).await?;
        Ok(messages)
    }
}
