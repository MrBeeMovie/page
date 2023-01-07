use std::{
    cmp, env,
    fs::File,
    io::{self, BufRead, Stdout, Write},
    path::PathBuf,
    process::exit,
};

use crossterm::{
    cursor::{self},
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
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
    col: usize,
    row: usize,
    ren_col: usize,
    ren_row: usize,
}

impl Cursor {
    fn new() -> Cursor {
        Cursor {
            col: 0,
            row: 0,
            ren_col: 0,
            ren_row: 0,
        }
    }

    fn handle_key_event(
        &mut self,
        stdout: &mut Stdout,
        key_event: KeyEvent,
        text: &Text,
    ) -> Result<(), io::Error> {
        match key_event.modifiers {
            KeyModifiers::NONE => match key_event.code {
                // quite
                KeyCode::Char('q') => {
                    fin_term(stdout)?;
                    exit(0);
                }

                // cursor movement
                _ => self.handle_cursor_move(stdout, key_event.code, text)?,
            },
            _ => (),
        }
        Ok(())
    }

    fn row_index(&self) -> usize {
        self.row + self.ren_row
    }

    fn col_index(&self) -> usize {
        self.col + self.ren_col
    }

    fn is_past_last_row(&self, text: &Text) -> bool {
        self.row_index() as isize >= text.rows.len() as isize - 1
    }

    fn is_past_row_end(&self, text: &Text) -> bool {
        let row_len = text.rows[self.row_index()].chars.len();
        self.col_index() as isize >= row_len as isize - 1
    }

    fn move_cursor_end_of_row(
        &mut self,
        stdout: &mut Stdout,
        text: &Text,
    ) -> Result<(), io::Error> {
        let row_len = text.rows[self.row_index()].chars.len();

        self.col = text.rows[self.row_index()].chars.len();
        self.ren_col = cmp::max(0, row_len as i16 - text.term_width as i16) as usize;

        execute!(stdout, cursor::MoveTo(self.col as u16, self.row as u16))?;

        Ok(())
    }

    fn handle_cursor_move(
        &mut self,
        stdout: &mut Stdout,
        key_code: KeyCode,
        text: &Text,
    ) -> Result<(), io::Error> {
        // if text is empty nowhere to move
        if text.rows.is_empty() {
            return Ok(());
        }

        match key_code {
            // UP
            KeyCode::Char('k') => {
                if self.row > 0 {
                    self.row -= 1;
                } else if self.ren_row > 0 {
                    self.ren_row -= 1;
                }
            }
            // DOWN
            KeyCode::Char('j') => {
                if !self.is_past_last_row(text) {
                    if self.row < text.term_height - 1 {
                        self.row += 1;
                    } else {
                        self.ren_row += 1;
                    }
                }
            }
            // LEFT
            KeyCode::Char('h') => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.ren_col > 0 {
                    self.ren_col -= 1;
                }
            }
            // RIGHT
            KeyCode::Char('l') => {
                if !self.is_past_row_end(text) {
                    if self.col < text.term_width - 1 {
                        self.col += 1;
                    } else {
                        self.ren_col += 1;
                    }
                }
            }
            _ => (),
        }

        // move cursor

        // if we are past last char in row move back to last char
        if self.is_past_row_end(text) {
            self.move_cursor_end_of_row(stdout, text)?;
        }

        execute!(stdout, cursor::MoveTo(self.col as u16, self.row as u16))?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Row {
    chars: Vec<char>,
}

#[derive(Debug)]
struct Text {
    term_width: usize,
    term_height: usize,
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

    fn draw_text(&self, stdout: &mut Stdout, cursor: &Cursor) -> Result<(), io::Error> {
        // save cursor position and hide
        execute!(stdout, cursor::SavePosition)?;
        execute!(stdout, cursor::Hide)?;

        // we need to render entire terminal screen
        for y in 0..self.term_height {
            let row_index = y + cursor.ren_row;
            let mut line = vec![' '; self.term_width];

            // only print part of row that is visible
            // rest we will print ' '
            if row_index < self.rows.len() && cursor.ren_col < self.rows[row_index].chars.len() {
                self.rows[row_index].chars[cursor.ren_col
                    ..cmp::min(
                        self.rows[row_index].chars.len(),
                        cursor.ren_col + self.term_width,
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
    let text = if args.len() > 1 {
        Text::from_file(args[1].to_owned().into())?
    } else {
        Text::new()?
    };

    // init Cursor struct
    let mut cursor = Cursor::new();

    loop {
        // draw
        text.draw_text(&mut stdout, &cursor)?;

        // handle input
        match read()? {
            Event::Key(key_event) => cursor.handle_key_event(&mut stdout, key_event, &text)?,
            _ => (),
        }
    }
}
