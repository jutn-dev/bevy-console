use bevy::prelude::*;

use crossterm::execute;
use crossterm::cursor::{DisableBlinking, MoveToColumn, MoveUp};
use crossterm::style::{Print, ResetColor, SetColors};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};

use shlex::Shlex;
use std::time::Duration;

use crate::console::{recompute_predictions, ConsoleCache};
use crate::{ConsoleCommandEntered, ConsoleConfiguration, ConsoleState};

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
pub(crate) fn commandline_input(
    mut state: ResMut<ConsoleState>,
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
                    if state.cursor_position != 0 {
                        //get char and its staring index
                        index = match state
                            .buf
                            .char_indices()
                            .nth(state.cursor_position - 1)
                        {
                            None => 0,
                            //add last char's len to get the correct position
                            Some(char) => char.0 + char.1.len_utf8(),
                        };
                    }
                    state.buf.insert(index, c);
                    state.cursor_position += 1;
                }
                crossterm::event::KeyCode::Backspace => {
                    if state.cursor_position < 1 {
                        continue;
                    }
                    let index = match state
                        .buf
                        .char_indices()
                        .nth(state.cursor_position - 1)
                    {
                        None => state.buf.len(),
                        //add last char's len to get the correct position
                        Some(char) => char.0,
                    };
                    state.buf.remove(index);
                    state.cursor_position -= 1;
                }
                crossterm::event::KeyCode::Left => {
                    if state.cursor_position == 0 {
                        continue;
                    }
                    state.cursor_position -= 1;
                }
                crossterm::event::KeyCode::Right => {
                    if state.cursor_position >= state.buf.chars().count() {
                        continue;
                    }
                    state.cursor_position += 1;
                }
                crossterm::event::KeyCode::Enter => {
                    state.cursor_position = 0;
                    handle_enter(
                        &mut state,
                        &config,
                        &mut command_entered,
                        &cache,
                    );
                }
                exit_key if exit_key == config.exit_key.0 => {
                    exit_event.write(AppExit::Success);
                    return;
                }
                crossterm::event::KeyCode::Up => {
                    if state.history.len() > 1
                        && state.history_index < state.history.len() - 1
                    {
                        if state.history_index == 0 && !state.buf.trim().is_empty()
                        {
                            //save buf to history
                            *state.history.get_mut(0).unwrap() = state.buf.clone();
                        }

                        state.history_index += 1;
                        let previous_item = state
                            .history
                            .get(state.history_index)
                            .unwrap()
                            .clone();
                        state.buf = previous_item.to_string();
                        state.cursor_position = state.buf.chars().count();
                    }
                }
                crossterm::event::KeyCode::Down => {
                    if state.history_index > 0 {
                        state.history_index -= 1;
                        let next_item = state
                            .history
                            .get(state.history_index)
                            .unwrap()
                            .clone();
                        state.buf = next_item.to_string();
                        state.cursor_position = state.buf.chars().count();
                    }
                }
                crossterm::event::KeyCode::Tab => {
                    handle_tab(&mut state, &config, &mut cache);
                }
                _ => (),
            }
        }
    }
}

pub(crate) fn update_terminal(
    console_state: Res<ConsoleState>,
    mut state: ResMut<ConsoleState>,
    config: Res<ConsoleConfiguration>,
) {
    let mut stdout = std::io::stdout();

    redraw_commandline(&state, &config);

    for line in console_state
        .scrollback
        .iter()
        .skip(state.scrollbacks_printed)
    {
        state.scrollbacks_printed += 1;
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
    state: &ConsoleState,
    config: &ConsoleConfiguration,
) {
    execute!(std::io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
    execute!(std::io::stdout(), MoveToColumn(0)).unwrap();
    execute!(
        std::io::stdout(),
        Print(format!("{}{}", config.symbol, state.buf))
    )
    .unwrap();

    execute!(
        std::io::stdout(),
        MoveToColumn((config.symbol.chars().count() + state.cursor_position) as u16)
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
    state: &mut ConsoleState,
    config: &ConsoleConfiguration,
    command_entered: &mut EventWriter<'_, ConsoleCommandEntered>,
    cache: &ConsoleCache,
) {
    //this code is almost the same as the egui console's

    // if we have a selected suggestion
    // replace the content of the buffer with it and set the cursor to the end
    if let Some(index) = state.suggestion_index {
        if index < cache.predictions_cache.len() && !cache.prediction_matches_buffer {
            state.buf = cache.predictions_cache[index].clone();
            state.suggestion_index = None;
            state.cursor_position = state.buf.chars().count();
            return;
        }
    }

    execute!(std::io::stdout(), Print("\r\n",)).unwrap();
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

                state
                    .scrollback
                    .push("error: Invalid command".into());
            }
        }

        state.buf.clear();
    }
}
