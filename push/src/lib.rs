use rad_terminal::args::Help;

pub const HELP: Help = Help {
    name: "push",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad push [--seed <host> | --seed-url <url>] [-f | --force]

Options

    --force, -f         Force push (default: false)
    --seed <host>       Use the given seed node for syncing
    --seed-url <url>    Use the given seed node URL for syncing
    --help              Print help
"#,
};
