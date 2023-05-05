#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
mod docker;
use crate::docker::{create_docker_file, docker_command};
use anyhow::Result;
use clap::{Parser, Subcommand};
use tempfile::NamedTempFile;
use tokio::{
    process::Command,
    signal::unix::{signal, SignalKind},
    spawn,
};
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

    #[clap(short = 'e', long)]
    pub env_file: Option<String>,

    #[clap(short = 'v', long)]
    pub volume: Option<String>,
}

#[derive(Subcommand)]
pub enum CliCommand {
    Build,
    Run,
}

struct MuslBuilder {
    args: Cli,
    docker_file: NamedTempFile,
}

impl Execute for MuslBuilder {
    async fn execute(self) -> Result<()> {
        match &self.args.command {
            CliCommand::Build => {
                self.create_docker_container("builder")
                    .await?
                    .extract_musl_binary()
                    .await?;
            }
            CliCommand::Run => {
                self.create_docker_container("runner")
                    .await?
                    .run_musl_binary()
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

    async fn create_docker_container(self, target: &str) -> Result<Self> {
        let tag = Uuid::new_v4().to_string();

        // Execute the docker build command
        docker_command!(
            "build",
            ".",
            "-f",
            self.docker_file.path(),
            "--target",
            target,
            "-t",
            &tag
        )
        .execute()
        .await?;

        let mut create_command = docker_command!(
            "create",
            "--name",
            &self.args.container_name,
            "-p",
            "9000:8080",
        );

        if let Some(env_file) = &self.args.env_file {
            create_command.arg("--env-file").arg(env_file);
        }

        if let Some(volume) = &self.args.volume {
            create_command.arg("--volume").arg(volume);
        }

        // add tag last in command
        create_command.arg(&tag);

        create_command.execute().await?;

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
        docker_command!("rm", &self.args.container_name);

        Ok(self)
    }
    async fn run_musl_binary(self) -> Result<()> {
        // Clone container name, so we can use it in spawned thread
        let container_name = self.args.container_name.clone();

        // spawn a thread to handle interrupt signal and clean up
        spawn(async move {
            let mut sigint = signal(SignalKind::interrupt()).unwrap();

            match sigint.recv().await {
                Some(()) => {
                    println!("Received SIGINT signal");
                    self.docker_file.close().unwrap();
                    docker_command!("rm", &container_name)
                        .execute()
                        .await
                        .unwrap();
                }
                None => eprintln!("Stream terminated before receiving SIGINT signal"),
            }
        });

        docker_command!("start", &self.args.container_name, "-a")
            .execute()
            .await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Cli::parse();

    MuslBuilder::new(args)?.execute().await?;

    Ok(())
}
