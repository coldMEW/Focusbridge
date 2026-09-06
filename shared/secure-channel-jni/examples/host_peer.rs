//! Disposable host test peer; fixed test keys must never be used by an application.
use focusbridge_secure_channel::{Identity, Session};
use std::io::{self, BufRead, Write};

fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    let desktop = Identity::from_private([8; 32]).unwrap();
    let phone = Identity::from_private([7; 32]).unwrap();
    let mut session = Session::desktop(&desktop, &[9; 32], [1; 16], phone.public_key()).unwrap();
    println!("{}", encode(&desktop.public_key()));
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let (command, hex) = line.split_once(' ').unwrap_or((&line, ""));
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let response = match command {
            "readHandshake" => {
                session.read_handshake(&bytes).unwrap();
                "ok".to_owned()
            }
            "writeHandshake" => encode(&session.write_handshake().unwrap()),
            "readConfirmation" => {
                session.read_confirmation(&bytes).unwrap();
                "ok".to_owned()
            }
            "writeConfirmation" => encode(&session.write_confirmation().unwrap()),
            "open" => match session.open_frame(&bytes).unwrap() {
                Some(record) => encode(&record),
                None => "partial".to_owned(),
            },
            "seal" => session
                .seal_record(&bytes)
                .unwrap()
                .iter()
                .map(|frame| encode(frame))
                .collect::<Vec<_>>()
                .join(","),
            _ => panic!("unknown test command"),
        };
        println!("{response}");
        io::stdout().flush().unwrap();
    }
}
