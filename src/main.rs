use bytes::Buf;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::cmp::min;
use std::fmt::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::{self, spawn};
use std::time::Duration;
use tungstenite::accept;

/// A simple program to transfer files between machines
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    sub_command: SubCommands,
}

#[derive(Debug, Subcommand)]
enum SubCommands {
    Listen,
    /// Send a file to the machine with the specified ID
    Send {
        file: PathBuf,
        /// Machine ID
        #[arg(short, long)]
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.sub_command {
        SubCommands::Listen => {
            println!("Listening on port 8101");
            let server = TcpListener::bind("127.0.0.1:8101").unwrap();
            for stream in server.incoming() {
                spawn(move || {
                    // let mut websocket = accept(stream.unwrap()).unwrap();
                    let mut downloaded = 0;
                    let mut buffer = tungstenite::buffer::ReadBuffer::<1024>::new();
                    let total_size = buffer.remaining();
                    let mut stream = stream.unwrap();
                    let pb = ProgressBar::new(total_size as u64);
                    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                        .unwrap()
                        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                        // .progress_chars("#>-")
                    );
                    loop {
                        let read_bytes = buffer.read_from(&mut stream).unwrap();

                        // let remaining = buffer.remaining();

                        if buffer.has_remaining() && downloaded < total_size {
                            let new = min(downloaded + read_bytes, total_size);
                            downloaded = new;
                            pb.set_position(new as u64);
                            thread::sleep(Duration::from_millis(12));

                            pb.finish_with_message("downloaded");
                        } else {
                        }

                        // let msg = websocket.read().unwrap();

                        // if msg.is_binary() {
                        //     let data = msg.into_data();
                        //     let total_size = data.len();

                        //     progress(total_size, &mut downloaded);

                        //     websocket
                        //         .send(tungstenite::Message::Text(String::from("Done!")))
                        //         .unwrap();
                        // } else if msg.is_text() {
                        //     websocket.send(msg).unwrap();
                        // } else if msg.is_close() {
                        //     let _ = websocket.close(None);
                        // }
                    }
                });
            }
        }
        SubCommands::Send { file, id } => {
            println!("Send {:#?} to {}", file, id);
            // let total_size = data.len();
            // let mut downloaded = 0;

            // progress(total_size, &mut downloaded);
        }
    }
}
