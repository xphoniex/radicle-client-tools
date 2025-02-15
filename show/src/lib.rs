use std::ffi::OsString;

use rad_terminal::args::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "show",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad show [<option>...]

Options

    --peer      Show device peer
    --project   Show current project
    --profile   Show current radicle profile
    --self      Show local user
    --ssh-key   Show current SSH key fingerprint
    --help      Print help
"#,
};

#[derive(Default, Eq, PartialEq)]
pub struct Options {
    pub show_peer_id: bool,
    pub show_self: bool,
    pub show_proj_id: bool,
    pub show_ssh_key: bool,
    pub show_profile_id: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut show_peer_id = false;
        let mut show_self = false;
        let mut show_proj_id = false;
        let mut show_profile_id = false;
        let mut show_ssh_key = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("peer") | Long("peer-id") => {
                    show_peer_id = true;
                }
                Long("self") | Long("self-id") => {
                    show_self = true;
                }
                Long("project") | Long("project-id") => {
                    show_proj_id = true;
                }
                Long("profile") | Long("profile-id") => {
                    show_profile_id = true;
                }
                Long("ssh-key") => {
                    show_ssh_key = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                show_self,
                show_peer_id,
                show_proj_id,
                show_ssh_key,
                show_profile_id,
            },
            vec![],
        ))
    }
}
