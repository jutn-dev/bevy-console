use bevy::prelude::*;
use crossterm::cursor::{DisableBlinking, MoveToColumn, MoveUp};
use crossterm::event::ModifierKeyCode;
use crossterm::execute;
use crossterm::style::{Print, ResetColor, SetColors};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use shlex::Shlex;
use std::time::Duration;

use crate::console::{recompute_predictions, ConsoleCache};
use crate::{ConsoleCommandEntered, ConsoleConfiguration, ConsoleState};

#[derive(Resource, Debug, Clone)]
pub(crate) struct CommandlineState {
    pub(crate) scrollbacks_printed: usize,
    ///cursor_position is the amout of inexes in the string not the amout of chars
    pub(crate) cursor_position: usize,

    //TODO
    //config options: move some where else

    //Why use crossterm `KeyCode` instead of bevy's?
    //All the terminal input is hadeled by crossterm so it's simpler to use crossterm's `KeyCode`s.
    pub exit_key: (crossterm::event::KeyCode, Option<ModifierKeyCode>),
}

impl Default for CommandlineState {
    fn default() -> Self {
        CommandlineState {
            scrollbacks_printed: 0,
            cursor_position: 0,
            exit_key: (
                crossterm::event::KeyCode::Esc,
                None,
            )
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
    mut commandline_state: ResMut<CommandlineState>,
    mut exit_event: EventWriter<AppExit>,
    mut command_entered: EventWriter<'_, ConsoleCommandEntered>,
    config: Res<ConsoleConfiguration>,
    mut cache: ResMut<ConsoleCache>,
) {
    while crossterm::event::poll(Duration::from_secs(0)).unwrap() {
        let events = crossterm::event::read().unwrap();
        if let crossterm::event::Event::Key(key) = events {
            //clear suggestions on event
            execute!(std::io::stdout(), Clear(ClearType::FromCursorDown)).unwrap();

            match key.code {
                crossterm::event::KeyCode::Char(c) => {
                    //finds the correct position to insert the char
                    let mut index = 0;
                    if commandline_state.cursor_position != 0 {
                        //get char and its staring index
                        index = match console_state
                            .buf
                            .char_indices()
                            .nth(commandline_state.cursor_position - 1)
                        {
                            None => 0,
                            //add last char's len to get the correct position
                            Some(char) => char.0 + char.1.len_utf8(),
                        };
                    }
                    console_state.buf.insert(index, c);
                    commandline_state.cursor_position += 1;
                }
                crossterm::event::KeyCode::Backspace => {
                    if commandline_state.cursor_position < 1 {
                        continue;
                    }
                    let index = match console_state
                        .buf
                        .char_indices()
                        .nth(commandline_state.cursor_position - 1)
                    {
                        None => console_state.buf.len(),
                        //add last char's len to get the correct position
                        Some(char) => char.0,
                    };
                    console_state.buf.remove(index);
                    commandline_state.cursor_position -= 1;
                }
                crossterm::event::KeyCode::Left => {
                    if commandline_state.cursor_position == 0 {
                        continue;
                    }
                    commandline_state.cursor_position -= 1;
                }
                crossterm::event::KeyCode::Right => {
                    if commandline_state.cursor_position >= console_state.buf.chars().count() {
                        continue;
                    }
                    commandline_state.cursor_position += 1;
                }
                crossterm::event::KeyCode::Enter => {
                    commandline_state.cursor_position = 0;
                    handle_enter(
                        &mut console_state,
                        &mut commandline_state,
                        &config,
                        &mut command_entered,
                        &cache,
                    );
                }
                exit_key if exit_key == commandline_state.exit_key.0 => {
                    exit_event.write(AppExit::Success);
                    return;
                }
                crossterm::event::KeyCode::Up => {
                    if console_state.history.len() > 1
                        && console_state.history_index < console_state.history.len() - 1
                    {
                        if console_state.history_index == 0 && !console_state.buf.trim().is_empty()
                        {
                            //save buf to history
                            *console_state.history.get_mut(0).unwrap() = console_state.buf.clone();
                        }

                        console_state.history_index += 1;
                        let previous_item = console_state
                            .history
                            .get(console_state.history_index)
                            .unwrap()
                            .clone();
                        console_state.buf = previous_item.to_string();
                        commandline_state.cursor_position = console_state.buf.chars().count();
                    }
                }
                crossterm::event::KeyCode::Down => {
                    if console_state.history_index > 0 {
                        console_state.history_index -= 1;
                        let next_item = console_state
                            .history
                            .get(console_state.history_index)
                            .unwrap()
                            .clone();
                        console_state.buf = next_item.to_string();
                        commandline_state.cursor_position = console_state.buf.chars().count();
                    }
                }
                crossterm::event::KeyCode::Tab => {
                    handle_tab(&mut console_state, &config, &mut cache);
                }
                _ => (),
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

    redraw_commandline(&commandline_state, &console_state, &config);

    for line in console_state
        .scrollback
        .iter()
        .skip(commandline_state.scrollbacks_printed)
    {
        commandline_state.scrollbacks_printed += 1;
        if line.trim().is_empty() {
            continue;
        }
        execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
        execute!(
            stdout,
            Print(format!("\r{}\r\n", line.replace('\n', "\r\n")))
        )
        .unwrap();
    }
}

///redraws the line where command is inputed
fn redraw_commandline(
    commandline_state: &CommandlineState,
    console_state: &ConsoleState,
    config: &ConsoleConfiguration,
) {
    execute!(std::io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
    execute!(std::io::stdout(), MoveToColumn(0)).unwrap();
    execute!(
        std::io::stdout(),
        Print(format!("{}{}", config.symbol, console_state.buf))
    )
    .unwrap();

    execute!(
        std::io::stdout(),
        MoveToColumn((config.symbol.chars().count() + commandline_state.cursor_position) as u16)
    )
    .unwrap();
}

fn handle_tab(
    console_state: &mut ConsoleState,
    config: &ConsoleConfiguration,
    cache: &mut ConsoleCache,
) {
    let mut stdout = std::io::stdout();
    recompute_predictions(console_state, cache, config.num_suggestions);

    if !cache.prediction_matches_buffer
        && !console_state.buf.is_empty()
        && !cache.predictions_cache.is_empty()
    {
        match &mut console_state.suggestion_index {
            Some(index) => {
                *index = (*index + 1) % cache.predictions_cache.len();
            }
            None => {
                console_state.suggestion_index = Some(0);
            }
        }
        //print suggestions
        for (i, suggestion) in cache.predictions_cache.iter().enumerate() {
            let is_highlighted = Some(i) == console_state.suggestion_index;

            execute!(stdout, Print("\r\n")).unwrap();
            if is_highlighted {
                execute!(
                    stdout,
                    SetColors(crossterm::style::Colors::new(
                        crossterm::style::Color::Black,
                        crossterm::style::Color::White
                    ))
                )
                .unwrap();
            }
            execute!(stdout, Print(suggestion)).unwrap();
            execute!(stdout, ResetColor).unwrap();
        }
        execute!(stdout, MoveUp(cache.predictions_cache.len() as u16)).unwrap();
    }
}

fn handle_enter(
    console_state: &mut ConsoleState,
    commandline_state: &mut CommandlineState,
    config: &ConsoleConfiguration,
    command_entered: &mut EventWriter<'_, ConsoleCommandEntered>,
    cache: &ConsoleCache,
) {
    //this code is almost the same as the egui console's

    // if we have a selected suggestion
    // replace the content of the buffer with it and set the cursor to the end
    if let Some(index) = console_state.suggestion_index {
        if index < cache.predictions_cache.len() && !cache.prediction_matches_buffer {
            console_state.buf = cache.predictions_cache[index].clone();
            console_state.suggestion_index = None;
            commandline_state.cursor_position = console_state.buf.chars().count();
            return;
        }
    }

    execute!(std::io::stdout(), Print("\r\n",)).unwrap();
    if console_state.buf.trim().is_empty() {
        console_state.scrollback.push(String::new());
    } else {
        let cmd_string = console_state.buf.clone();
        console_state.history.insert(1, cmd_string);
        if console_state.history.len() > config.history_size + 1 {
            console_state.history.pop_back();
        }
        console_state.history_index = 0;

        let mut args = Shlex::new(&console_state.buf).collect::<Vec<_>>();

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

                console_state
                    .scrollback
                    .push("error: Invalid command".into());
            }
        }

        console_state.buf.clear();
    }
}
