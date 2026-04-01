use std::process::Command;

pub fn scrub_command(command: &mut Command) -> &mut Command {
    command
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_CLI_PROFILE")
}

pub fn scrubbed_command(program: &str) -> Command {
    let mut command = Command::new(program);
    scrub_command(&mut command);
    command
}
