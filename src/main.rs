use anyhow::Result;
use clap::Parser;
use std::include_str;
use std::io::Write;
use tempfile::NamedTempFile;
use tera::{Context, Tera};
use tokio::process::Command;

#[derive(Parser)]
#[clap(version, author, about)]
pub struct Cli {
    /// Directory to use as root of project
    #[clap(short = 'p', long, default_value = ".")]
    pub path: String,

    #[clap(short = 'b', long)]
    pub bin: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a Tera instance
    let mut tera = Tera::default();

    // Add the Dockerfile template
    tera.add_raw_template("Dockerfile", include_str!("../Dockerfile"))?;

    // Parse command line arguments
    let args = Cli::parse();

    // Create a temporary file for Dockerfile
    let mut temp_docker_file = NamedTempFile::new_in(".")?;

    // Add the path and bin to the a Tera context
    let mut context = Context::new();
    context.insert("path", &args.path);
    context.insert("bin", &args.bin);

    // Render the Dockerfile template
    let docker_file = tera.render("Dockerfile", &context)?;

    // Write Dockerfile to temporary file
    writeln!(temp_docker_file, "{}", docker_file)?;

    // Spawn the docker build command
    let mut cmd = Command::new("docker")
        .arg("build")
        .arg(".")
        .arg("-f")
        .arg(temp_docker_file.path())
        .spawn()
        .expect("failed to spawn");

    // Await until the command completes
    let status = cmd.wait().await?;

    println!("the command exited with: {}", status);
    Ok(())
}
