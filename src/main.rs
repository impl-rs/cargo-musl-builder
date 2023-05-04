use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::include_str;
use std::io::BufReader;
use std::io::Write;
use tempfile::NamedTempFile;
use tera::{Context, Tera};
use tokio::process::Command;
use uuid::Uuid;
use zip::ZipArchive;

const MUSL_FILE: &str = "bootstrap.zip";

macro_rules! docker_command {
    ($($arg:expr),* $(,)?) => ({
        let mut cmd = Command::new("docker");
        $( cmd.arg($arg); )*
        cmd.spawn().expect("failed to spawn").wait().await?
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

impl MuslBuilder {
    fn new(args: Cli) -> Result<Self> {
        // Create a temporary Dockerfile
        let docker_file = create_docker_file(&args)?;

        Ok(Self { args, docker_file })
    }
    async fn execute(self) -> Result<()> {
        match &self.args.command {
            CliCommand::Build => self.execute_docker_commands().await?,
            CliCommand::Run => {
                self.execute_docker_commands()
                    .await?
                    .run_musl_binary()
                    .await?
            }
        };
        Ok(())
    }
    async fn execute_docker_commands(self) -> Result<Self> {
        let tag = Uuid::new_v4().to_string();

        // Execute the docker build command
        docker_command!("build", ".", "-f", self.docker_file.path(), "-t", &tag);

        // Create the container
        docker_command!("create", "--name", &self.args.container_name, &tag);

        // Copy out the bootstrap.zip
        docker_command!(
            "cp",
            format!("lambda:/opt/app/{}", MUSL_FILE),
            &self.args.output_path
        );

        // Remove the container
        docker_command!("rm", &self.args.container_name);

        Ok(self)
    }
    async fn run_musl_binary(self) -> Result<Self> {
        let file_path = format!("{}/{}", &self.args.output_path, MUSL_FILE);
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        ZipArchive::new(reader)?.extract(&self.args.output_path)?;

        dbg!("here");
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
