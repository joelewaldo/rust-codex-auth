mod commands;
mod rows;
mod select;
#[cfg(test)]
mod test_support;

use std::{fmt, io};

use crate::{
    auth,
    storage::{self, StorageError},
    usage::RemoteUsageFetcher,
};

use commands::{
    complete_login, interactive_switch, list_accounts, remove_account, run_codex_login,
    switch_account,
};

#[derive(Debug)]
pub enum CliError {
    Message(String),
    Io(io::Error),
    Auth(auth::AuthError),
    Storage(StorageError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => write!(f, "{message}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Auth(err) => write!(f, "{err}"),
            Self::Storage(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<auth::AuthError> for CliError {
    fn from(value: auth::AuthError) -> Self {
        Self::Auth(value)
    }
}

impl From<StorageError> for CliError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

pub fn run(args: Vec<String>) -> Result<(), CliError> {
    match args.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [command] if command == "help" || command == "--help" || command == "-h" => {
            print_help();
            Ok(())
        }
        [command] if command == "list" => {
            let home = storage::resolve_codex_home()?;
            let fetcher = RemoteUsageFetcher;
            list_accounts(&home, &fetcher)
        }
        [command] if command == "login" => {
            let home = run_codex_login(false)?;
            complete_login(&home)
        }
        [command, flag] if command == "login" && flag == "--device-auth" => {
            let home = run_codex_login(true)?;
            complete_login(&home)
        }
        [command] if command == "switch" => {
            let home = storage::resolve_codex_home()?;
            interactive_switch(&home)
        }
        [command, selector] if command == "switch" => {
            let home = storage::resolve_codex_home()?;
            switch_account(&home, selector)
        }
        [command, selector] if command == "remove" => {
            let home = storage::resolve_codex_home()?;
            remove_account(&home, selector)
        }
        [command] if command == "remove" => Err(CliError::Message(
            "`remove` requires a row number or exact email".to_string(),
        )),
        [command, ..] => Err(CliError::Message(format!("unknown command `{command}`"))),
    }
}

fn print_help() {
    println!(
        "\
codex-auth

Usage:
  codex-auth login [--device-auth]
  codex-auth list
  codex-auth switch [row|email]
  codex-auth remove <row|email>"
    );
}
