#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::include_str;
use std::io::Write;
use tempfile::NamedTempFile;
use tera::{Context, Tera};
use tokio::{
    process::Command,
    signal::unix::{signal, SignalKind},
    spawn,
};
use uuid::Uuid;

pub trait Execute {
    async fn execute(self) -> Result<()>;
}

impl Execute for Command {
    async fn execute(mut self) -> Result<()> {
        self.spawn().expect("failed to spawn").wait().await?;
        Ok(())
    }
}

const MUSL_FILE: &str = "bootstrap.zip";

macro_rules! docker_command {
    ($($arg:expr),* $(,)?) => ({
        let mut cmd = Command::new("docker");
        $( cmd.arg($arg); )*
        cmd
    });
}

macro_rules! context {
    ($([$key:expr, $arg:expr]),* $(,)?) => ({
        let mut context = Context::new();
        $( context.insert($key, $arg); )*
        context
    });
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

fn create_docker_file(args: &Cli) -> Result<NamedTempFile> {
    // Create a Tera instance
    let mut tera = Tera::default();

    // Add the Dockerfile template
    tera.add_raw_template("Dockerfile", include_str!("../Dockerfile"))?;

    // Create a temporary file for Dockerfile
    let mut temp_docker_file = NamedTempFile::new_in(".")?;

    // Add the path and bin to the a Tera context
    let context = context!(["path", &args.path], ["bin", &args.bin]);

    // Render the Dockerfile template
    let docker_file = tera.render("Dockerfile", &context)?;

    // Write Dockerfile to temporary file
    writeln!(temp_docker_file, "{}", docker_file)?;

    Ok(temp_docker_file)
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
        // Create file reader for bootstrap.zip
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
