# ADR-013: CLI Framework

**Status:** Accepted

**Context:** mononium-cli has two roles (node daemon + wallet). The CLI framework must support subcommands, argument parsing, and shell completion.

**Decision:** clap with derive macros.

- `#[derive(Parser)]` for main command structure
- `#[derive(Subcommand)]` for nested commands (node, wallet, query)
- `clap_complete` for shell completion generation

**Consequences:**

- Industry standard Rust CLI framework
- Declarative argument parsing via derives
- Auto-generated help text
- Shell completions for bash/zsh/fish/powershell
- Slightly larger binary than minimal frameworks
