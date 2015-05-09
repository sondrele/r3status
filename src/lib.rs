extern crate rustc_serialize;
use rustc_serialize::{json, Decodable, Decoder, Encodable, Encoder};

use std::path::Path;
use std::io::{self, Error, ErrorKind, LineWriter, Write, BufRead, BufReader};
use std::process::{self, Command, Child, ChildStdout};

#[derive(Debug, PartialEq)]
pub enum Alignment {
    Right,
    Left,
    Center,
}

impl Decodable for Alignment {
    fn decode<D: Decoder>(d: &mut D) -> Result<Alignment, D::Error> {
        match &try!(d.read_str())[..] {
            "right"  => Ok(Alignment::Right),
            "left"   => Ok(Alignment::Left),
            "center" => Ok(Alignment::Center),
            other    => Err(d.error(&format!("`{}` is not a valid alignment", other))),
        }
    }
}

impl Encodable for Alignment {
    fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_str(match *self {
            Alignment::Right => "right",
            Alignment::Left => "left",
            Alignment::Center => "center",
        })
    }
}


#[derive(Debug, RustcDecodable, RustcEncodable)]
pub struct Block {
    pub full_text: String,
    pub short_text: Option<String>,
    pub color: Option<String>,
    pub min_width: Option<usize>,
    pub align: Option<Alignment>,
    pub urgent: Option<bool>,
    pub name: Option<String>,
    pub instance: Option<String>,
    pub separator: Option<bool>,
    pub separator_block_width: Option<usize>,
}

impl Default for Block {
    fn default() -> Block {
        Block {
            full_text: String::new(),
            short_text: None,
            color: None,
            min_width: None,
            align: None,
            urgent: None,
            name: None,
            instance: None,
            separator: None,
            separator_block_width: None,
        }
    }
}

#[derive(Debug, RustcDecodable, RustcEncodable)]
pub struct Header {
    version: usize,
    stop_signal: Option<usize>,
    cont_signal: Option<usize>,
    click_events: Option<bool>,
}


impl Default for Header {
    fn default() -> Header {
        Header {
            version: 1,
            stop_signal: None,
            cont_signal: None,
            click_events: None,
        }
    }
}
pub struct R3Status {
    config_file: Option<String>,
    status_line: Vec<Block>,
    reader: Option<BufReader<ChildStdout>>,
    writer: LineWriter<io::Stdout>,
    buffer: String,
}

impl R3Status {
    pub fn new() -> R3Status {
        R3Status {
            config_file: None,
            status_line: Vec::new(),
            reader: None,
            writer: LineWriter::new(io::stdout()),
            buffer: String::new(),
        }
    }

    pub fn config_file(&mut self, config: &str) {
        self.config_file = Some(config.to_string());
    }

    pub fn clear(&mut self) {
        self.buffer.clear()
    }

    pub fn read_line(&mut self) -> io::Result<usize> {
        if let Some(reader) = self.reader.as_mut() {
            // trust i3status to give us valid utf8 for now...
            unsafe {
                reader.read_until('\n' as u8, self.buffer.as_mut_vec())
            }
        } else {
            Err(Error::new(ErrorKind::Other, "A reader has not been set for the process"))
        }
    }

    pub fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.ends_with("\n") {
            try!(write!(self.writer, "{}", self.buffer))
        } else {
            try!(writeln!(self.writer, "{}", self.buffer))
        }
        self.clear();
        Ok(())
    }

    pub fn write_str(&mut self, line: &str) -> io::Result<()> {
        self.writer.write_all(line.as_bytes())
    }

    pub fn write_msg(&mut self, msg: &str) -> io::Result<()> {
        let m = Block { full_text: msg.to_string(), .. Default::default()};
        self.buffer = json::encode(&vec![m]).unwrap();

        try!(self.flush_buffer());
        self.write_str(",")
    }

    pub fn pipe_header(&mut self) -> io::Result<()> {
        try!(self.read_line());

        let mut h: Header = json::decode(&self.buffer).unwrap();
        h.click_events = Some(true);
        self.buffer = json::encode(&h).unwrap();
        self.flush_buffer()
    }

    pub fn pipe_line(&mut self) -> io::Result<()> {
        try!(self.read_line());
        self.flush_buffer()
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut i3s = try!(spawn_i3status(self.config_file.as_ref()));

        if let Some(i3out) = i3s.stdout {
            self.reader = Some(BufReader::new(i3out));

            try!(self.pipe_header());
            // Pipe the start of the infinate array
            try!(self.pipe_line());
            // Pipe the first line, this is the only line that is not prefixed with `,`
            try!(self.pipe_line());

            loop {
                try!(self.pipe_line());
            }
        } else {
            println!("Failed to aquire handle to i3status' `stdout`");
            println!("Killling i3status...");
            i3s.kill()
        }
    }
}

pub fn run() {
    let mut r3 = R3Status::new();

    if let Err(e) = r3.run() {
        println!("Failed to spawn r3status: {:?}", e);
    }
}

fn spawn_i3status<P: AsRef<Path>>(_config: Option<P>) -> io::Result<Child> {
    Command::new("i3status")
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::inherit())
        .spawn()
}

#[test]
fn test_encode_decode_alignment() {
    assert_eq!(r#""right""#, json::encode(&Alignment::Right).unwrap());
    assert_eq!(r#""left""#, json::encode(&Alignment::Left).unwrap());
    assert_eq!(r#""center""#, json::encode(&Alignment::Center).unwrap());

    assert_eq!(Ok(Alignment::Right), json::decode(r#""right""#));
    assert_eq!(Ok(Alignment::Left), json::decode(r#""left""#));
    assert_eq!(Ok(Alignment::Center), json::decode(r#""center""#));
}
