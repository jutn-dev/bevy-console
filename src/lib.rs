#![doc = include_str ! ("../README.md")]
#![deny(missing_docs)]

use bevy::prelude::*;
pub use bevy_console_derive::ConsoleCommand;
use bevy_egui::{EguiContextPass, EguiPlugin, EguiPreUpdateSet};
use console::{block_keyboard_input, block_mouse_input, ConsoleCache};
use trie_rs::TrieBuilder;

use crate::commandline::{cleanup_commandline, commandline, init_commandline, update_terminal, CommandlineState};
use crate::commands::clear::{clear_command, ClearCommand};
use crate::commands::exit::{exit_command, ExitCommand};
use crate::commands::help::{help_command, HelpCommand};
pub use crate::console::{
    AddConsoleCommand, Command, ConsoleCommand, ConsoleCommandEntered, ConsoleConfiguration,
    ConsoleOpen, NamedCommand, PrintConsoleLine,
};
pub use crate::log::*;

use crate::console::{console_ui, receive_console_line, ConsoleState};
pub use clap;

// mod color;
mod color;
mod commands;
mod console;
mod log;
mod macros;
mod commandline;
/// Console plugin.
pub struct ConsolePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
/// The SystemSet for console/command related systems
pub enum ConsoleSet {
    /// Systems configuring commands at startup, only once
    Startup,

    /// Systems operating the console UI (the input layer)
    ConsoleUI,

    /// Systems executing console commands (the functionality layer).
    /// All command handler systems are added to this set
    Commands,

    /// Systems running after command systems, which depend on the fact commands have executed beforehand (the output layer).
    /// For example a system which makes use of [`PrintConsoleLine`] events should be placed in this set to be able to receive
    /// New lines to print in the same frame
    PostCommands,
}

/// Run condition which does not run any command systems if no command was entered
fn have_commands(commands: EventReader<ConsoleCommandEntered>) -> bool {
    !commands.is_empty()
}

/// builds the predictive search engine for completions
fn init(config: Res<ConsoleConfiguration>, mut cache: ResMut<ConsoleCache>) {
    let mut trie_builder = TrieBuilder::new();
    for cmd in config.commands.keys() {
        trie_builder.push(cmd);
    }

    for completions in &config.arg_completions {
        trie_builder.push(completions.join(" "));
    }

    cache.commands_trie = Some(trie_builder.build());
}

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleConfiguration>()
            .init_resource::<ConsoleState>()
            .init_resource::<ConsoleOpen>()
            .init_resource::<ConsoleCache>()
            .add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .add_console_command::<ClearCommand, _>(clear_command)
            .add_console_command::<ExitCommand, _>(exit_command)
            .add_console_command::<HelpCommand, _>(help_command)
            // after per-command startup
            .add_systems(Startup, init.after(ConsoleSet::Startup))
            .add_systems(
                PreUpdate,
                (block_mouse_input, block_keyboard_input)
                    .after(EguiPreUpdateSet::ProcessInput)
                    .before(EguiPreUpdateSet::BeginPass),
            )
            .add_systems(
                EguiContextPass,
                (
                    console_ui.in_set(ConsoleSet::ConsoleUI),
                    receive_console_line.in_set(ConsoleSet::PostCommands),
                ),
            )
            .configure_sets(
                EguiContextPass,
                (
                    ConsoleSet::Commands
                        .after(ConsoleSet::ConsoleUI)
                        .run_if(have_commands),
                    ConsoleSet::PostCommands.after(ConsoleSet::Commands),
                ),
            );

        // Don't initialize an egui plugin if one already exists.
        // This can happen if another plugin is using egui and was installed before us.
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin {
                enable_multipass_for_primary_context: true,
            });
        }
    }
}


///commandline Plugin is used when you want to have console inside terminal
pub struct CommandlinePlugin;

impl Plugin for CommandlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleConfiguration>()
            .init_resource::<ConsoleState>()
            .init_resource::<ConsoleOpen>()
            .init_resource::<ConsoleCache>()
            .init_resource::<CommandlineState>()
            .add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .add_console_command::<ClearCommand, _>(clear_command)
            .add_console_command::<ExitCommand, _>(exit_command)
            .add_console_command::<HelpCommand, _>(help_command)
            // after per-command startup
            .add_systems(Startup, init.after(ConsoleSet::Startup))
            .add_systems(Startup, init_commandline.after(ConsoleSet::Startup))
            .add_systems(Last, cleanup_commandline)
            
            //TODO change thease to commandline ones
            /*
            .add_systems(
                PreUpdate,
                (block_mouse_input, block_keyboard_input)
                    .after(EguiPreUpdateSet::ProcessInput)
                    .before(EguiPreUpdateSet::BeginPass),
            )
            */
            .add_systems(
                Update,
                (
                    update_terminal.in_set(ConsoleSet::ConsoleUI),
                    commandline.in_set(ConsoleSet::ConsoleUI),
                    receive_console_line.in_set(ConsoleSet::PostCommands),
                ),
            )
            .configure_sets(
                EguiContextPass,
                (
                    ConsoleSet::Commands
                        .after(ConsoleSet::ConsoleUI)
                        .run_if(have_commands),
                    ConsoleSet::PostCommands.after(ConsoleSet::Commands),
                ),
            );

    }
}
