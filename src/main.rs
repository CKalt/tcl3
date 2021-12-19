use std::io::prelude::*;
use std::net::{TcpStream,Shutdown};
use serde_json::Value;
use structopt::StructOpt;
use std::fs;
use std::str;
use std::io::ErrorKind;
use ctrlc;
use std::sync::Arc;

const DEFAULT_REQUEST_PORT:  &str = "8080";
const DEFAULT_RESPONSE_PORT: &str = "8081";

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(default_value = DEFAULT_REQUEST_PORT, short = "i", long = "input-port")]
    input_port: u32,
    #[structopt(default_value = DEFAULT_RESPONSE_PORT, short = "o", long = "output-port")]
    output_port: u32,
    #[structopt(default_value = "localhost", short = "s", long = "host")]
    host: String,
    #[structopt(short = "r", long = "request")]
    request_file: Option<String>,
    #[structopt(short = "p", long = "pretty-json")]
    pretty_json: bool,
}

fn get_url(host: &String, port: u32) -> String {
    format!("{}:{}", host, port)
}

fn handle_client(mut request_stream: TcpStream, opt: Opt)
            -> std::io::Result<()> {

    let sample_request : String = 
        if let Some(sample_request) = opt.request_file {
                fs::read_to_string(sample_request).unwrap()
        } else {
        r#"
{
   "command" : "FETCH",
   "parameters" : [
      "REPLAY",
      "archive_211015_123637"
   ]
}
"#.trim().to_string()
        };

    let rq_json = serde_json::from_str::<Value>(&sample_request[..]).unwrap();

    if opt.pretty_json {
        let rq_json_text = serde_json::to_string_pretty(&rq_json).unwrap();
        println!("request(pretty validated)={}", rq_json_text);
    }

    println!("request(orig)={}", sample_request);

    // Note we send the original request that has been validated
    // by this point in the code.

    let req_len = sample_request.len();
    let len_msg = format!("{:08x}", req_len);
    println!("TP001: req-strm write <length message>: msg={} len={}",
        len_msg, req_len);
    let wresult = request_stream.write(len_msg.as_bytes());

    match wresult {
        Err(e) => {
            println!("error writing len_msg: e={}", e);
        }
        _ => {}
    }

    println!("TP002: req-stm write <sample message>: msg={} len={}",
                sample_request, sample_request.len());
    let wresult = request_stream.write(sample_request.as_bytes());
    match wresult {
        Err(e) => {
            println!("error writing rq_json_text: {}", e);
        }
        _ => {}
    }
    println!("TP002.1: write returned");

    println!("TP003: resp-strm connect");
    let response_stream =
        match TcpStream::connect(get_url(&opt.host, opt.output_port)) {
            Ok(response) => response,
            Err(e) => {
                eprintln!("connect failed on output_port e = {:?}", e);
                std::process::exit(1);
            }
        };
    println!("TP003.1: connected to response_stream");

    // ctrlc handler
    let response_stream = Arc::new(response_stream);
    let handle = Arc::clone(&response_stream);
    ctrlc::set_handler(move || {
        let _ = handle.shutdown(Shutdown::Read);
    }).unwrap();

    let mut read_count = 100;

    let mut tp_i = 4;
    loop {
        // read len msg
        let mut len_buf: [u8; 8] = [0; 8];
        println!("TP00{}: (a) calling read_exact called {} bytes from\n\
                  response stream",
                        tp_i,
                        len_buf.len());

        if let Err(e) = (&*response_stream).read_exact(&mut len_buf) {
            if e.kind() == ErrorKind::UnexpectedEof {
                panic!("ctrl c pressed e={:?}", e);
            }
            else {
                panic!("Error while attempting to read len_msg e={:?}", e);
            }
        }

        println!("TP00{}.1: (b) 8 bytes read = {:?}", tp_i, len_buf);
        let len_str = str::from_utf8(&len_buf).unwrap();
        let bytes_to_read: usize
            = usize::from_str_radix(len_str.trim(), 16).unwrap();
        println!("TP00{}.2: (c) converts to hex str={} or bytes_to_read={}",
            tp_i, len_str, bytes_to_read);

        tp_i += 1;

        // read exactly `bytes_to_read` len and error if not 
        // valid json
        let mut response_buf = vec![0u8; bytes_to_read];
        println!("TP00{}: (d) read_exact len={}", tp_i, response_buf.len());
        (&*response_stream).read_exact(&mut response_buf).unwrap();
        let response = str::from_utf8(&response_buf).unwrap();

        println!("TP00{}.1: (e) {} bytes received=[{}]", tp_i,
            response_buf.len(), response);

        let rsp_json =
            serde_json::from_str::<Value>(response).unwrap();

        if opt.pretty_json {
            let rsp_json_text = serde_json::to_string_pretty(&rsp_json).unwrap();
            println!("TP00{}.2: (f) response(pretty validated)={}", tp_i, rsp_json_text);
        }

        read_count -= 1;
        if read_count == 0 {
            break;
        }

        tp_i += 1;
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    println!("opt={:?}", opt);

    let url = get_url(&opt.host, opt.input_port);
    println!("TP000: connecting to req-strm");
    let request_stream =
        match TcpStream::connect(&url[..]) {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("connect failed on input port {} {}", url, err);
                std::process::exit(1);
            }
        };

    handle_client(request_stream, opt)?;
    println!("Done");
    Ok(())
}
