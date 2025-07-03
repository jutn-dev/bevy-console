use bevy::prelude::*;
use crossterm::cursor::{DisableBlinking, MoveToColumn, RestorePosition, SavePosition};
use crossterm::event::{KeyModifiers, ModifierKeyCode};
use crossterm::style::Print;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, DisableLineWrap, EnableLineWrap,
};
use crossterm::{cursor, execute};
use shlex::Shlex;
use std::io::stdin;
use std::time::Duration;

use crate::{ConsoleCommandEntered, ConsoleConfiguration, ConsoleState};

#[derive(Resource, Debug, Clone)]
pub(crate) struct CommandlineState {
    pub(crate) scrollbacks_printed: usize,
    //Why use crossterm `KeyCode` instead of bevy's?
    //All the terminal input is hadeled by crossterm so it's simpler to use crossterm's `KeyCode`s.
    pub exit_key: Vec<crossterm::event::KeyCode>,
}

impl Default for CommandlineState {
    fn default() -> Self {
        CommandlineState {
            scrollbacks_printed: 0,
            exit_key: vec![
                crossterm::event::KeyCode::Modifier(ModifierKeyCode::LeftControl),
                crossterm::event::KeyCode::Char('c'),
            ],
        }
    }
}

pub(crate) fn init_commandline() {
    enable_raw_mode().expect("Terminal doesn't support raw mode.");
    execute!(std::io::stdout(), DisableBlinking).unwrap();
}

pub(crate) fn cleanup_commandline(mut exit_event: EventReader<AppExit>) {
    for _ in exit_event.read() {
        disable_raw_mode().expect("Failed to disable raw mode.");
        print!("\r\n");
    }
}
pub(crate) fn commandline(
    mut console_state: ResMut<ConsoleState>,
    mut exit_event: EventWriter<AppExit>,
    mut command_entered: EventWriter<'_, ConsoleCommandEntered>,
    config: Res<ConsoleConfiguration>,
) {
    while crossterm::event::poll(Duration::from_secs(0)).unwrap() {
        let events = crossterm::event::read().unwrap();
        if let crossterm::event::Event::Key(key) = events {
            if let crossterm::event::KeyCode::Char(c) = key.code {
                console_state.buf.push(c);
            }
            if key.code == crossterm::event::KeyCode::Backspace {
                console_state.buf.pop();
            }
            if key.code == crossterm::event::KeyCode::Enter {
                handle_enter(&mut console_state, &config, &mut command_entered);
            }
            if key.code == crossterm::event::KeyCode::Esc {
                exit_event.write(AppExit::Success);
                return;
            }
        }
    }
}

pub(crate) fn update_terminal(
    console_state: Res<ConsoleState>,
    mut commandline_state: ResMut<CommandlineState>,
    config: Res<ConsoleConfiguration>,
) {
    let mut stdout = std::io::stdout();

    redraw_commandline(&console_state, &config);
    
    for line in console_state
        .scrollback
        .iter()
        .skip(commandline_state.scrollbacks_printed)
    {
        commandline_state.scrollbacks_printed += 1;
        execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
        execute!(stdout, Print(format!("{}\r\n", line.replace('\n', "\r\n")))).unwrap();
    }
}

///redraws the line where command is inputed
fn redraw_commandline(state: &ConsoleState, config: &ConsoleConfiguration) {
    execute!(std::io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
    execute!(std::io::stdout(), MoveToColumn(0)).unwrap();
    execute!(
        std::io::stdout(),
        Print(format!("{}{}", config.symbol, state.buf))
    )
    .unwrap();
    execute!(
        std::io::stdout(),
        MoveToColumn((config.symbol.chars().count() + state.buf.chars().count()) as u16)
    )
    .unwrap();
}

fn handle_enter(
    state: &mut ConsoleState,
    config: &ConsoleConfiguration,
    command_entered: &mut EventWriter<'_, ConsoleCommandEntered>,
) {
    execute!(std::io::stdout(), Print("\r\n",)).unwrap();
    
    //this code is almost the same as the console's 
    if state.buf.trim().is_empty() {
        state.scrollback.push(String::new());
    } else {
        let cmd_string = state.buf.clone();
        state.history.insert(1, cmd_string);
        if state.history.len() > config.history_size + 1 {
            state.history.pop_back();
        }
        state.history_index = 0;

        let mut args = Shlex::new(&state.buf).collect::<Vec<_>>();

        if !args.is_empty() {
            let command_name = args.remove(0);
            debug!("Command entered: `{command_name}`, with args: `{args:?}`");

            let command = config.commands.get(command_name.as_str());

            if command.is_some() {
                command_entered.write(ConsoleCommandEntered { command_name, args });
            } else {
                debug!(
                    "Command not recognized, recognized commands: `{:?}`",
                    config.commands.keys().collect::<Vec<_>>()
                );

                state.scrollback.push("error: Invalid command".into());
            }
        }

        state.buf.clear();
    }
}
