# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

@AGENTS.md

`AGENTS.md` is the shared source of truth for all agents: fork policy, commands, architecture and conventions. Put new shared rules there, and keep only Claude Code-specific notes in this file.

## Claude Code Notes

- `pnpm tauri dev` never exits. Run it as a background command with its output redirected to a log file in the scratchpad, then wait on the log (for example until it prints ``Running `target``) instead of blocking the shell.
- Each PowerShell tool call runs in a new process. `Add-Type` definitions and `$env:` changes do not carry over, so define helpers and set variables in the same call that uses them.
- The shell inherits the PATH that existed when Claude Code started. A toolchain installed during the session (for example Rust in `%USERPROFILE%\.cargo\bin`) has to be prepended to `$env:Path`/`PATH` explicitly until Claude Code is restarted.
