#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
mod docker;
use crate::docker::{create_docker_file, docker_command};
use anyhow::Result;
use clap::{Parser, Subcommand};
use tempfile::NamedTempFile;
use tokio::process::Command;
use uuid::Uuid;

const MUSL_FILE: &str = "bootstrap.zip";

pub trait Execute {
    async fn execute(self) -> Result<()>;
}

impl Execute for Command {
    async fn execute(mut self) -> Result<()> {
        self.spawn().expect("failed to spawn").wait().await?;
        Ok(())
    }
}

#[derive(Parser)]
#[clap(version, author, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: CliCommand,
    /// Directory to use as root of project
    #[clap(short = 'p', long, default_value = ".")]
    pub path: String,

    #[clap(short = 'b', long)]
    pub bin: String,

    #[clap(long, default_value = ".")]
    pub output_path: String,

    #[clap(short = 'c', long, default_value = "lambda")]
    pub container_name: String,
}

#[derive(Subcommand)]
pub enum CliCommand {
    Build,
}

struct MuslBuilder {
    args: Cli,
    docker_file: NamedTempFile,
}

impl Execute for MuslBuilder {
    async fn execute(self) -> Result<()> {
        match &self.args.command {
            CliCommand::Build => {
                self.create_docker_container()
                    .await?
                    .extract_musl_binary()
                    .await?;
            }
        };
        Ok(())
    }
}

impl MuslBuilder {
    fn new(args: Cli) -> Result<Self> {
        // Create a temporary Dockerfile
        let docker_file = create_docker_file(&args)?;

        Ok(Self { args, docker_file })
    }

    async fn create_docker_container(self) -> Result<Self> {
        let tag = Uuid::new_v4().to_string();

        let is_ci = match std::env::var("CI") {
            Ok(val) => val == "true",
            Err(_e) => false,
        };

        // Execute the docker build command with cache on CI for Github Actions
        if is_ci {
            docker_command!(
                "build",
                ".",
                "-f",
                self.docker_file.path(),
                "-t",
                &tag,
                "--cache-to",
                "type=gha,mode=max",
                "--cache-from",
                "type=gha"
            )
            .execute()
            .await?;
        } else {
            docker_command!("build", ".", "-f", self.docker_file.path(), "-t", &tag,)
                .execute()
                .await?;
        }

        docker_command!("create", "--name", &self.args.container_name, &tag)
            .execute()
            .await?;

        Ok(self)
    }
    async fn extract_musl_binary(self) -> Result<Self> {
        // Copy out the bootstrap.zip
        docker_command!(
            "cp",
            format!("lambda:/opt/app/{}", MUSL_FILE),
            &self.args.output_path
        )
        .execute()
        .await?;

        // Remove the container
        docker_command!("rm", &self.args.container_name)
            .execute()
            .await?;

        Ok(self)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Cli::parse();

    MuslBuilder::new(args)?.execute().await?;

    Ok(())
}
