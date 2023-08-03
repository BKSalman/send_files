use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::cmp::min;
use std::fmt::Write;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write as IoWrite};
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread::spawn;
use std::{mem, process};

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
        // #[arg(short, long)]
        // id: String,
        #[arg(long)]
        ip: SocketAddrV4,
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
                    let mut downloaded = 0;
                    let mut buffer = tungstenite::buffer::ReadBuffer::<1024>::new();
                    let mut stream = stream.unwrap();

                    let mut size_buffer = [0; mem::size_of::<u64>()];
                    stream.read_exact(&mut size_buffer).unwrap();
                    let total_size = u64::from_be_bytes(size_buffer);

                    let pb = ProgressBar::new(total_size as u64);

                    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                        .unwrap()
                        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                        // .progress_chars("#>-")
                    );

                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/home/salman/Downloads/lmao.something")
                        .unwrap();
                    loop {
                        let read_bytes = buffer.read_from(&mut stream).unwrap();

                        if read_bytes == 0 {
                            pb.abandon();
                            println!("Download finished!");
                            file.write_all(&buffer.into_vec()).unwrap();
                            break;
                        }

                        if downloaded < total_size {
                            let new = min(downloaded + read_bytes as u64, total_size);
                            downloaded = new;
                            pb.set_position(new as u64);
                        }
                    }
                });
            }
        }
        SubCommands::Send { file, ip } => {
            println!("Sending {:#?}", file);
            let mut stream = TcpStream::connect(ip).unwrap();

            if !file.is_file() {
                println!("path is not a file");
                process::exit(1);
            }

            let metadata = fs::metadata(file).unwrap();
            let file = File::open(file).unwrap();
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::with_capacity(metadata.len() as usize);

            let read_bytes = reader.read_to_end(&mut buffer).unwrap();

            if read_bytes == 0 {
                println!("File is empty");
                process::exit(1);
            }

            let mut file_size_be = metadata.len().to_be_bytes().to_vec();

            file_size_be.extend(buffer);

            stream.write(&file_size_be).unwrap();

            // let total_size = data.len();
            // let mut downloaded = 0;

            // progress(total_size, &mut downloaded);
        }
    }
}
