use crate::Cli;
use anyhow::Result;
use std::include_str;
use std::io::Write;
use tempfile::NamedTempFile;
use tera::{Context, Tera};

macro_rules! docker_command {
    ($($arg:expr),* $(,)?) => ({
        let mut cmd = Command::new("docker");
        $( cmd.arg($arg); )*
        cmd
    });
}

pub(crate) use docker_command;

macro_rules! context {
    ($([$key:expr, $arg:expr]),* $(,)?) => ({
        let mut context = Context::new();
        $( context.insert($key, $arg); )*
        context
    });
}

pub fn create_docker_file(args: &Cli) -> Result<NamedTempFile> {
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
