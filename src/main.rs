#![feature(std_misc, fs, net, io, env, path, collections)]

extern crate gridui;

use std::net::{TcpListener, TcpStream};
use std::thread::Thread;
use std::fs::File;
use std::io::{Read, Write};
use std::io;
use std::env;
use std::u32;

use gridui::glyphcode;

fn read_exactly<R: Read>(source: &mut R, len: usize) -> io::Result<Vec<u8>> {
    println!("Attempting to read {} bytes", len);
    let mut dest : Vec<u8> = Vec::new();
    dest.resize(len, 0u8);
    let mut read_count: usize = 0;
    while read_count < len {
        let this_read_count = try!(source.read(&mut dest[read_count..len]));
        if this_read_count==0 {
            return Err(io::Error::new(io::ErrorKind::Other, "Not enough bytes to read", None));
        }
        read_count += this_read_count;
        println!("Did a read. Read count is now {}", read_count);
    }
    println!("Reading complete!");
    Ok( dest )
}

fn decode_u16_le(bs: &[u8]) -> u16 {
    return (bs[0] as u16)
        | ((bs[1] as u16) << 8);
}

fn encode_u32_le(dest: &mut[u8], x: u32) {
    dest[0] = ((x>> 0) & 0xff) as u8;
    dest[1] = ((x>> 8) & 0xff) as u8;
    dest[2] = ((x>>16) & 0xff) as u8;
    dest[3] = ((x>>24) & 0xff) as u8;
}


fn handle_invalid_url(mut _stream: TcpStream) -> io::Result<()>  {
    Ok( () )
}

fn pack_u8s_to_u32s(bs: Vec<u8>) -> Vec<u32> {
    (&bs[]).chunks(4).map(|chunk| {
      (chunk[0] as u32) | ((chunk[1] as u32)<<8) | ((chunk[2] as u32)<<8) | ((chunk[3] as u32)<<8)
    }).collect()
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    let working_dir = env::current_dir().unwrap();
    let static_root = working_dir.join("static");
    let handler_root = working_dir.join("handler");

    let url_glyph_count = decode_u16_le(&try!(read_exactly(&mut stream, 2))[]) as usize;
    let url_byte_count = url_glyph_count * 4;
    println!("Going to read a url occupying {} glyphs ({} bytes)", url_glyph_count, url_byte_count);
    let url : Vec<u32> = pack_u8s_to_u32s(try!(read_exactly(&mut stream, url_byte_count)));
    
    println!("Url glyphcodes: {:?}", url);
    
    let slash = glyphcode::from_char('/').unwrap();
    let dot = glyphcode::from_char('.').unwrap();
    
    let path_start = match url.position_elem(&slash) {
        Some(pos) => pos+1,
        None => url.len()
    };
    let path = &url[path_start..];
    
    //if contains_dotdot(&path) {
        //return handle_invalid_url(stream);
    //}
    
    let mut file_path = static_root;
    
    for path_component_glyphs in path.split(|x| { *x == slash }) {
        let path_component : String = match glyphcode::to_string(path_component_glyphs) {
            Some(s) => { s },
            None => { return handle_invalid_url(stream); }
        };
        if path_component.chars().all(|c| { c == '.' }) {
            return handle_invalid_url(stream);
        }
        
        // No exotic characters allowed!
        if path_component.chars().any(|c| { c=='\\' || c <= (0x20 as char) || c>= (0x80 as char)}) {
            return handle_invalid_url(stream);
        }
        
        file_path.push(path_component);
    }
    
    println!("Path: {:?}", file_path);
    
    let dot_position = match path.rposition_elem(&dot) {
        Some(dot_position) => dot_position,
        None => { return handle_invalid_url(stream); }
    };
    let after_dot = &path[dot_position+1..];
    
    let after_dot_str = if let Some(after_dot_str) = glyphcode::to_string(after_dot) {
        after_dot_str
    } else {
        return handle_invalid_url(stream);
    };
    
    println!("Extension: {:?}", after_dot_str);
    
    let mut handler_path = handler_root;
    handler_path.push(after_dot_str);
    println!("{:?}", handler_path);
    let mut handler_file = try!(File::open(&handler_path));
    let mut handler_bytes = Vec::new();
    try!(handler_file.read_to_end(&mut handler_bytes));
    
    let mut content_file = try!(File::open(&file_path));
    let mut content_bytes = Vec::new();
    try!(content_file.read_to_end(&mut content_bytes));
    
    if content_bytes.len()<20 {
        content_bytes.resize(20, 0u8);
    }
    if (handler_bytes.len() as u64) + (content_bytes.len() as u64) > u32::MAX as u64 {
        return handle_invalid_url(stream);
    }
    
    let handler_byte_len = handler_bytes.len();
    let content_byte_len = content_bytes.len();
    encode_u32_le(&mut handler_bytes[12.. 16], handler_byte_len as u32);
    encode_u32_le(&mut handler_bytes[16.. 20], content_byte_len as u32);
    let mut length_buffer = [0u8; 4];
    encode_u32_le(&mut length_buffer, (handler_bytes.len() + content_bytes.len()) as u32);
    
    try!(stream.write_all(&length_buffer));
    try!(stream.write_all(&handler_bytes[]));
    try!(stream.write_all(&content_bytes[]));
    
    //let mut code_file = File::open("page.bin").ok().expect("Failed to open page.bin");
    //let mut contents = Vec::new();
    //code_file.read_to_end(&mut contents).unwrap_or_else(|_| { panic!("Could not read MIPS code"); } );
    
    Ok( () )
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:5692").ok().expect("Could not initialize TCP listener");

    
    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                Thread::spawn(move|| {
                    if let Err(e) = handle_client(stream) {
                        println!("Error in handle_client: {:?}", e);
                    }
                });
            }
            Err(e) => { println!("Connecting failed: {:?}", e); }
        }
    }

    // close the socket server
    drop(listener);
}