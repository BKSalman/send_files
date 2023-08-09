use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::cmp::min;
use std::fmt::Write;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write as IoWrite};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread::spawn;
use std::time::Duration;
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
    Listen {
        #[arg(long, short)]
        port: Option<u16>,
    },
    /// Send a file to the machine with the specified ID
    Send {
        #[clap(name = "FILE")]
        file_name: PathBuf,

        #[arg(long)]
        ip: IpAddr,

        #[arg(long, short)]
        port: Option<u16>,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.sub_command {
        SubCommands::Listen { port } => {
            let port = port.unwrap_or(8000);

            println!("Listening on port {port}");

            let server =
                TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port))
                    .unwrap();

            for stream in server.incoming() {
                spawn(move || {
                    let mut downloaded = 0;
                    let mut buffer = tungstenite::buffer::ReadBuffer::<1024>::new();
                    let mut stream = stream.unwrap();
                    println!("incoming connection from {:#?}", stream.peer_addr());

                    let mut size_buffer = [0; mem::size_of::<u64>()];
                    stream.read_exact(&mut size_buffer).unwrap();
                    let total_size = u64::from_be_bytes(size_buffer);

                    let mut filename_size = [0; mem::size_of::<u64>()];
                    stream.read_exact(&mut filename_size).unwrap();
                    let filename_size = u64::from_be_bytes(filename_size);
                    let mut filename_buf = vec![0; filename_size as usize];
                    stream.read_exact(&mut filename_buf).unwrap();
                    let filename = String::from_utf8(filename_buf).unwrap();

                    println!("file size: {}", total_size);
                    println!("file name: {}", filename);

                    let pb = ProgressBar::new(total_size);

                    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                        .unwrap()
                        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
                        // .progress_chars("#>-")
                    );

                    let path = get_next_path(PathBuf::from(filename));

                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
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
                            pb.set_position(new);
                        }
                    }
                });
            }
        }
        SubCommands::Send {
            file_name: file_path,
            ip,
            port,
        } => {
            let port = port.unwrap_or(8000);

            println!("Connecting to {ip} with port: {port}");

            let mut stream =
                TcpStream::connect_timeout(&SocketAddr::new(*ip, port), Duration::from_secs(5))
                    .unwrap();

            println!("Connected to {ip}");

            if !file_path.is_file() {
                println!("path is not a file");
                process::exit(1);
            }

            let metadata = fs::metadata(file_path).unwrap();
            let file = File::open(file_path).unwrap();
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::with_capacity(metadata.len() as usize);

            let read_bytes = reader.read_to_end(&mut buffer).unwrap();

            if read_bytes == 0 {
                println!("File is empty");
                process::exit(1);
            }

            println!("file size: {}", metadata.len());

            let file_size_be = metadata.len().to_be_bytes().to_vec();

            println!("Sending file size to {}", ip);

            stream.write_all(&file_size_be).unwrap();

            // send file name size

            let filename = file_path.file_name().expect("file should have a name");

            let filename_size = filename.len() as u64;
            let filename_size = filename_size.to_be_bytes().to_vec();

            stream.write_all(&filename_size).unwrap();

            // send file name

            // TODO: does this need to be ordered in big endian before sending??
            stream
                .write_all(filename.to_string_lossy().as_bytes())
                .unwrap();

            println!("Sending {:#?} to {}", file_path, ip);

            stream.write_all(&buffer).unwrap();

            // let total_size = data.len();
            // let mut downloaded = 0;

            // progress(total_size, &mut downloaded);
        }
    }
}

fn get_next_path(mut path: PathBuf) -> PathBuf {
    let regex = regex::Regex::new(r" \(\d\)").expect("Failed to compile regex");

    let mut count = 1;
    while path.exists() {
        let mut file_extensions = vec![];
        while let Some(extension) = path.extension() {
            println!("extension: {extension:#?}");
            file_extensions.push(format!("{}", extension.to_str().expect("extension str")));
            path = path.with_extension("");
        }

        let file_name = path
            .file_name()
            .expect("Only files can be transferred")
            .to_string_lossy()
            .to_string();

        let next_file_name = if let Some(_last_match) = regex.find(&file_name) {
            let replacement = format!(" ({})", count);
            let replaced = regex.replace(&file_name, replacement.as_str());
            count += 1;
            replaced.into_owned()
        } else {
            let mut new_name = file_name.clone();
            new_name.push_str(" (1)");
            new_name
        };

        if file_extensions.is_empty() {
            path.set_file_name(next_file_name);
        } else {
            path.set_file_name(format!(
                "{}.{}",
                next_file_name,
                file_extensions
                    .into_iter()
                    .rev()
                    .collect::<Vec<String>>()
                    .join(".")
            ));
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::get_next_path;

    #[test]
    fn test_get_next_path() {
        let temp_dir = std::env::temp_dir();

        let mut test = temp_dir.clone();
        test.push("something");
        fs::write(&test, "test").unwrap();

        let mut expected = temp_dir.clone();

        expected.push("something (1)");

        assert_eq!(expected, get_next_path(test.clone()));
        let _ = fs::remove_file(&test);
    }

    #[test]
    fn test_multiple_files_get_next_path() {
        let temp_dir = std::env::temp_dir();

        let mut test = temp_dir.clone();
        test.push("something");
        fs::write(&test, "test").unwrap();

        let mut test_1 = temp_dir.clone();
        test_1.push("something (1)");
        fs::write(&test_1, "test (1)").unwrap();

        let mut expected = temp_dir.clone();
        expected.push("something (2)");

        assert_eq!(expected, get_next_path(test.clone()));
        let _ = fs::remove_file(test_1);
        let _ = fs::remove_file(test);
    }

    #[test]
    fn test_multiple_files_with_gap_get_next_path() {
        let temp_dir = std::env::temp_dir();

        let mut test = temp_dir.clone();
        test.push("something");
        fs::write(&test, "test").unwrap();

        let mut test_2 = temp_dir.clone();
        test_2.push("something (2)");
        fs::write(&test_2, "test (2)").unwrap();

        let mut expected = temp_dir.clone();

        expected.push("something (1)");

        assert_eq!(expected, get_next_path(test.clone()));
        let _ = fs::remove_file(&test);
        let _ = fs::remove_file(&test_2);
    }

    #[test]
    fn test_multiple_files_with_gap_with_extension_get_next_path() {
        let temp_dir = std::env::temp_dir();

        let mut test = temp_dir.clone();
        test.push("something.txt");
        fs::write(&test, "test").unwrap();

        let mut test_2 = temp_dir.clone();
        test_2.push("something (2).txt");
        fs::write(&test_2, "test (2)").unwrap();

        let mut expected = temp_dir.clone();

        expected.push("something (1).txt");

        assert_eq!(expected, get_next_path(test.clone()));
        let _ = fs::remove_file(&test);
        let _ = fs::remove_file(&test_2);
    }

    #[test]
    fn test_multiple_files_with_gap_with_multiple_extensions_get_next_path() {
        let temp_dir = std::env::temp_dir();

        let mut test = temp_dir.clone();
        test.push("something.txt.test");
        fs::write(&test, "test").unwrap();

        let mut test_2 = temp_dir.clone();
        test_2.push("something (2).txt.test");
        fs::write(&test_2, "test (2)").unwrap();

        let mut expected = temp_dir.clone();

        expected.push("something (1).txt.test");

        assert_eq!(expected, get_next_path(test.clone()));
        let _ = fs::remove_file(&test);
        let _ = fs::remove_file(&test_2);
    }
}
