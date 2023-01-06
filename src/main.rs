use std::{
    cmp, env,
    fs::File,
    io::{self, BufRead, Stdout, Write},
    path::PathBuf,
};

use crossterm::{
    cursor::{self},
    event::{read, Event, KeyCode, KeyModifiers},
    execute, queue,
    style::{self},
    terminal::{self, Clear},
};

fn init_term(stdout: &mut Stdout) -> Result<(), io::Error> {
    // enable raw mode
    terminal::enable_raw_mode()?;

    // clear terminal
    execute!(stdout, Clear(terminal::ClearType::All))?;

    // move cursor to top left
    execute!(stdout, cursor::MoveTo(0, 0))?;

    Ok(())
}

fn fin_term(stdout: &mut Stdout) -> Result<(), io::Error> {
    // disable raw mode
    terminal::disable_raw_mode()?;

    // clear terminal
    execute!(stdout, Clear(terminal::ClearType::All))?;

    // move cursor to top left
    execute!(stdout, cursor::MoveTo(0, 0))?;

    Ok(())
}

#[derive(Default, Debug)]
struct Cursor {
    x: usize,
    y: usize,
    ren_x: usize,
    ren_y: usize,
}

impl Cursor {
    fn new() -> Cursor {
        Cursor {
            x: 0,
            y: 0,
            ren_x: 0,
            ren_y: 0,
        }
    }
}

#[derive(Default, Clone, Debug)]
struct Row {
    chars: Vec<char>,
}

#[derive(Default, Debug)]
struct Text {
    term_width: usize,
    term_height: usize,
    cursor: Cursor,
    rows: Vec<Row>,
}

impl Text {
    fn new() -> Result<Text, io::Error> {
        let (term_width, term_height) = terminal::size()?;
        let (term_width, term_height) = (term_width.into(), term_height.into());

        Ok(Text {
            term_width,
            term_height,
            rows: vec![],
            cursor: Cursor::new(),
        })
    }

    fn from_file(path: PathBuf) -> Result<Text, io::Error> {
        let mut text = Text::new()?;

        let file = File::open(path)?;
        let buf_reader = io::BufReader::new(file);

        buf_reader.lines().for_each(|line| {
            let mut line = line.unwrap_or_default();

            // pop line endings
            if line.ends_with('\n') {
                line.pop();

                if line.ends_with('\r') {
                    line.pop();
                }
            }

            // write to Text struct
            text.rows.push(Row {
                chars: line.chars().collect(),
            });
        });

        Ok(text)
    }

    fn draw_text(&self, stdout: &mut Stdout) -> Result<(), io::Error> {
        // save cursor position and hide
        execute!(stdout, cursor::SavePosition)?;
        execute!(stdout, cursor::Hide)?;

        // we need to render entire terminal screen
        for y in 0..self.term_height {
            let row_index = y + self.cursor.ren_y;
            let mut line = vec![' '; self.term_width];

            // only print part of row that is visible
            // rest we will print ' '
            if row_index < self.rows.len() && self.cursor.ren_x < self.rows[row_index].chars.len() {
                self.rows[row_index].chars[self.cursor.ren_x
                    ..cmp::min(
                        self.rows[row_index].chars.len(),
                        self.cursor.ren_x + self.term_width,
                    )]
                    .iter()
                    .enumerate()
                    .for_each(|(i, c)| line[i] = *c);
            }

            queue!(stdout, cursor::MoveTo(0, y.try_into().unwrap()))?;
            queue!(stdout, style::Print(line.iter().collect::<String>()))?;
        }

        stdout.flush()?;

        // restore cursor position and show
        execute!(stdout, cursor::RestorePosition)?;
        execute!(stdout, cursor::Show)?;

        Ok(())
    }
}

fn main() -> Result<(), io::Error> {
    // get handle to stdout()
    let mut stdout = io::stdout();

    init_term(&mut stdout)?;

    // get args
    let args = env::args().collect::<Vec<String>>();

    // init Text struct
    let mut text = if args.len() > 1 {
        Text::from_file(args[1].to_owned().into())?
    } else {
        Text::new()?
    };

    loop {
        // draw
        text.draw_text(&mut stdout)?;

        // handle input
        match read()? {
            Event::Key(event) => match event.modifiers {
                KeyModifiers::NONE => match event.code {
                    // quite
                    KeyCode::Char('q') => break,

                    // move
                    KeyCode::Char('k') => {
                        if text.cursor.y > 0 {
                            text.cursor.y -= 1;

                            let row_len = text.rows[text.cursor.y + text.cursor.ren_y].chars.len();

                            // if we are past last char in row move back to last char
                            if text.cursor.x + text.cursor.ren_x >= row_len {
                                text.cursor.x =
                                    text.rows[text.cursor.y + text.cursor.ren_y].chars.len();
                                text.cursor.ren_x =
                                    cmp::max(0, row_len as i16 - text.term_width as i16) as usize;
                            }

                            execute!(
                                stdout,
                                cursor::MoveTo(text.cursor.x as u16, text.cursor.y as u16)
                            )?;
                        } else if text.cursor.ren_y > 0 {
                            text.cursor.ren_y -= 1;
                        }
                    }
                    KeyCode::Char('j') => {
                        if text.cursor.y < text.term_height {
                            text.cursor.y += 1;

                            let row_len = text.rows[text.cursor.y + text.cursor.ren_y].chars.len();

                            // if we are past last char in row move back to last char
                            if text.cursor.x + text.cursor.ren_x >= row_len {
                                text.cursor.x =
                                    text.rows[text.cursor.y + text.cursor.ren_y].chars.len();
                                text.cursor.ren_x =
                                    cmp::max(0, row_len as i16 - text.term_width as i16) as usize;
                            }

                            execute!(
                                stdout,
                                cursor::MoveTo(text.cursor.x as u16, text.cursor.y as u16)
                            )?;
                        } else {
                            text.cursor.ren_y += 1;
                        }
                    }
                    KeyCode::Char('h') => {
                        if text.cursor.x > 0 {
                            text.cursor.x -= 1;
                            execute!(stdout, cursor::MoveLeft(1))?;
                        } else if text.cursor.ren_x > 0 {
                            text.cursor.ren_x -= 1;
                        }
                    }
                    KeyCode::Char('l') => {
                        if (text.cursor.x + text.cursor.ren_x)
                            < text.rows[text.cursor.y + text.cursor.ren_y].chars.len()
                        {
                            if text.cursor.x < text.term_width {
                                text.cursor.x += 1;
                                execute!(stdout, cursor::MoveRight(1))?;
                            } else {
                                text.cursor.ren_x += 1;
                            }
                        }
                    }

                    // edit
                    KeyCode::Char(c) => {
                        text.rows[Into::<usize>::into(text.cursor.y)].chars
                            [Into::<usize>::into(text.cursor.x)] = c
                    }

                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
    }

    fin_term(&mut stdout)?;

    Ok(())
}
