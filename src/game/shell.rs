#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCommand {
    Cd(String),
    Ls,
    Cat(String),
    Grep(String),
    Rm(String),
    Sudo(String),
    Jetpack,
    GitStash,
    Help,
    Exit,
    Invalid(String),
}

pub fn parse(input: &str) -> ShellCommand {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ShellCommand::Invalid(String::new());
    }
    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let arg = parts.collect::<Vec<_>>().join(" ");

    match cmd {
        "cd" => ShellCommand::Cd(arg),
        "ls" => ShellCommand::Ls,
        "cat" => ShellCommand::Cat(arg),
        "grep" => ShellCommand::Grep(arg),
        "rm" => ShellCommand::Rm(arg),
        "sudo" => ShellCommand::Sudo(arg),
        "jetpack" => ShellCommand::Jetpack,
        "git" if arg == "stash" => ShellCommand::GitStash,
        "help" | "?" => ShellCommand::Help,
        "exit" | "quit" => ShellCommand::Exit,
        other => ShellCommand::Invalid(other.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cd() {
        assert_eq!(parse("cd north"), ShellCommand::Cd("north".into()));
    }

    #[test]
    fn parses_ls() {
        assert_eq!(parse("ls"), ShellCommand::Ls);
    }

    #[test]
    fn parses_git_stash() {
        assert_eq!(parse("git stash"), ShellCommand::GitStash);
    }

    #[test]
    fn empty_is_invalid() {
        assert_eq!(parse("   "), ShellCommand::Invalid(String::new()));
    }

    #[test]
    fn unknown_command() {
        assert_eq!(parse("foobar"), ShellCommand::Invalid("foobar".into()));
    }
}
