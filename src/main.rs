use std::{
    cmp, env,
    fs::File,
    io::{self, BufRead, Stdout, Write},
    path::PathBuf,
    time::Duration,
};

use crossterm::{
    cursor::{self},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{self},
    terminal::{self, Clear},
};

struct TermInfo {
    alive: bool,
    stdout: Stdout,
    width: usize,
    height: usize,
}

impl TermInfo {
    fn new() -> Result<TermInfo, io::Error> {
        // get handle to stdout()
        let mut stdout = io::stdout();

        // enable raw mode
        terminal::enable_raw_mode()?;

        // clear terminal
        execute!(stdout, Clear(terminal::ClearType::All))?;

        // move cursor to top left
        execute!(stdout, cursor::MoveTo(0, 0))?;

        // get term dimensions
        let (term_width, term_height) = terminal::size()?;

        Ok(TermInfo {
            alive: true,
            stdout,
            width: term_width as usize,
            height: term_height as usize,
        })
    }

    fn update_size(&mut self) -> Result<(), io::Error> {
        let (term_width, term_height) = terminal::size()?;

        self.width = term_width as usize;
        self.height = term_height as usize;

        Ok(())
    }
}

impl Drop for TermInfo {
    fn drop(&mut self) {
        // disable raw mode
        terminal::disable_raw_mode().unwrap();

        // clear terminal
        execute!(self.stdout, Clear(terminal::ClearType::All)).unwrap();

        // move cursor to top left
        execute!(self.stdout, cursor::MoveTo(0, 0)).unwrap();
    }
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
        key_event: KeyEvent,
        term_info: &mut TermInfo,
        text: &Text,
    ) -> Result<(), io::Error> {
        if key_event.modifiers == KeyModifiers::NONE {
            match key_event.code {
                // quite
                KeyCode::Char('q') => {
                    term_info.alive = false;
                    return Ok(());
                }

                // cursor movement
                _ => self.handle_cursor_move(key_event.code, term_info, text)?,
            }
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
        self.col_index() as isize >= row_len as isize
    }

    fn move_cursor_end_of_row(
        &mut self,
        term_info: &mut TermInfo,
        text: &Text,
    ) -> Result<(), io::Error> {
        let row_len = text.rows[self.row_index()].chars.len();

        self.col = text.rows[self.row_index()].chars.len();
        self.ren_col = cmp::max(0, row_len as i16 - term_info.width as i16) as usize;

        execute!(
            term_info.stdout,
            cursor::MoveTo(self.col as u16, self.row as u16)
        )?;

        Ok(())
    }

    fn handle_cursor_move(
        &mut self,
        key_code: KeyCode,
        term_info: &mut TermInfo,
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

                // if we are past last char in row move back to last char
                if self.is_past_row_end(text) {
                    self.move_cursor_end_of_row(term_info, text)?;
                }
            }
            // DOWN
            KeyCode::Char('j') => {
                if !self.is_past_last_row(text) {
                    if self.row < term_info.height - 1 {
                        self.row += 1;
                    } else {
                        self.ren_row += 1;
                    }
                }

                // if we are past last char in row move back to last char
                if self.is_past_row_end(text) {
                    self.move_cursor_end_of_row(term_info, text)?;
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
                    if self.col < term_info.width - 1 {
                        self.col += 1;
                    } else {
                        self.ren_col += 1;
                    }
                }
            }
            _ => (),
        }

        // move cursor
        execute!(
            term_info.stdout,
            cursor::MoveTo(self.col as u16, self.row as u16)
        )?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Row {
    chars: Vec<char>,
}

#[derive(Debug)]
struct Text {
    rows: Vec<Row>,
}

impl Text {
    fn new() -> Result<Text, io::Error> {
        Ok(Text { rows: vec![] })
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

    fn draw_text(&self, term_info: &mut TermInfo, cursor: &Cursor) -> Result<(), io::Error> {
        // save cursor position and hide
        execute!(term_info.stdout, cursor::SavePosition)?;
        execute!(term_info.stdout, cursor::Hide)?;

        // we need to render entire terminal screen
        for y in 0..term_info.height {
            let row_index = y + cursor.ren_row;
            let mut line = vec![' '; term_info.width];

            // only print part of row that is visible
            // rest we will print ' '
            if row_index < self.rows.len() && cursor.ren_col < self.rows[row_index].chars.len() {
                self.rows[row_index].chars[cursor.ren_col
                    ..cmp::min(
                        self.rows[row_index].chars.len(),
                        cursor.ren_col + term_info.width,
                    )]
                    .iter()
                    .enumerate()
                    .for_each(|(i, c)| line[i] = *c);
            }

            queue!(term_info.stdout, cursor::MoveTo(0, y.try_into().unwrap()))?;
            queue!(
                term_info.stdout,
                style::Print(line.iter().collect::<String>())
            )?;
        }

        term_info.stdout.flush()?;

        // restore cursor position and show
        execute!(term_info.stdout, cursor::RestorePosition)?;
        execute!(term_info.stdout, cursor::Show)?;

        Ok(())
    }
}

fn main() -> Result<(), io::Error> {
    // init TermInfo struct
    let mut term_info = TermInfo::new()?;

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

    while term_info.alive {
        // update term size
        term_info.update_size()?;

        // draw
        text.draw_text(&mut term_info, &cursor)?;

        // handle input
        if poll(Duration::from_millis(500))? {
            if let Event::Key(key_event) = read()? {
                cursor.handle_key_event(key_event, &mut term_info, &text)?;
            }
        }
    }

    Ok(())
}
