#![deny(warnings, clippy::pedantic)]

use bhttp::{Message, Mode};
use std::fs::File;
use std::io;
use std::io::Read;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use futures::{stream, StreamExt};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct HexArg(Vec<u8>);
impl FromStr for HexArg {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(HexArg)
    }
}
impl Deref for HexArg {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, StructOpt)]
#[allow(dead_code)]
#[structopt(name = "ohttp-client", about = "Make an oblivious HTTP request.")]
struct Args {
    /// The URL of an oblivious proxy resource.
    /// If you use an oblivious request resource, this also works, though
    /// you don't get any of the privacy guarantees.
    url: String,
    /// A hexadecimal version of the key configuration for the target URL.
    config: HexArg,

    /// Where to read request content.
    /// If you omit this, input is read from `stdin`.
    #[structopt(long, short = "i")]
    input: Option<PathBuf>,

    /// Where to write response content.
    /// If you omit this, output is written to `stdout`.
    #[structopt(long, short = "o")]
    output: Option<PathBuf>,

    /// Read and write as binary HTTP messages instead of text.
    #[structopt(long, short = "b")]
    binary: bool,

    /// When creating message/bhttp, use the indefinite-length form.
    #[structopt(long, short = "n")]
    indefinite: bool,

    /// Concurrency
    #[structopt(long, short = "c")]
    concurrency: usize,

    /// Requests
    #[structopt(long, short = "r")]
    requests: usize,

    /// Enable override for the trust store.
    #[structopt(long)]
    trust: Option<PathBuf>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Res<()> {
    let args = Args::from_args();
    ::ohttp::init();
    let _ = env_logger::try_init();

    let request = if let Some(infile) = &args.input {
        let mut r = io::BufReader::new(File::open(infile)?);
        if args.binary {
            Message::read_bhttp(&mut r)?
        } else {
            Message::read_http(&mut r)?
        }
    } else {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;
        let mut r = io::Cursor::new(buf);
        if args.binary {
            Message::read_bhttp(&mut r)?
        } else {
            Message::read_http(&mut r)?
        }
    };

    let concurrent_requests:usize = args.concurrency;
    let total_requests:usize = args.requests;
    let urls = vec![&args.url; total_requests];

    // let mut request_buf = Vec::new();
    // request.write_bhttp(Mode::KnownLength, &mut request_buf)?;
    // let ohttp_request = ohttp::ClientRequest::new(&args.config)?;
    // let (enc_request, _ohttp_response) = ohttp_request.encapsulate(&request_buf)?;
    // println!("Request: {}", hex::encode(&enc_request));

    let client = match &args.trust {
        Some(pem) => {
            let mut buf = Vec::new();
            File::open(pem)?.read_to_end(&mut buf)?;
            let cert = reqwest::Certificate::from_pem(buf.as_slice())?;
            reqwest::ClientBuilder::new()
                .add_root_certificate(cert)
                .build()?
        }
        None => reqwest::ClientBuilder::new().build()?,
    };

    let bodies = stream::iter(urls)
        .map(|url| {
            let client = &client;
            let mut request_buf = Vec::new();
            request.write_bhttp(Mode::KnownLength, &mut request_buf).expect("YAY");
            let ohttp_request = ohttp::ClientRequest::new(&args.config);
            let Ok((enc_request, _ohttp_response)) = ohttp_request.expect("REASON").encapsulate(&request_buf)  else { todo!() };
            println!("Request: {}", hex::encode(&enc_request));
            async move {
                let resp = client
                    .post(url)
                    .header("content-type", "message/ohttp-req")
                    .body(enc_request)
                    .send()
                    .await?;

                resp.bytes().await
            }
        })
        .buffer_unordered(concurrent_requests);


    // let enc_response = client
    //     .post(&args.url)
    //     .header("content-type", "message/ohttp-req")
    //     .body(enc_request)
    //     .send()
    //     .await?
    //     .error_for_status()?
    //     .bytes()
    //     .await?;

    bodies
        .for_each(|b| async {
            match b {
                Ok(b) => println!("Got {} bytes", b.len()),
                Err(e) => eprintln!("Got an error: {}", e),
            }
        })
        .await;
         Ok(())
    // println!("Response: {}", hex::encode(&enc_response));
    // let response_buf = ohttp_response.decapsulate(&enc_response)?;
    // let response = Message::read_bhttp(&mut std::io::Cursor::new(&response_buf[..]))?;

    // let mut output: Box<dyn io::Write> = if let Some(outfile) = &args.output {
    //     Box::new(File::open(outfile)?)
    // } else {
    //     Box::new(std::io::stdout())
    // };
    // if args.binary {
    //     response.write_bhttp(args.mode(), &mut output)?;
    // } else {
    //     response.write_http(&mut output)?;
    // }
    // Ok(())

    // println!("Response: {}", hex::encode(&enc_response));
    // let response_buf = ohttp_response.decapsulate(&enc_response)?;
    // let response = Message::read_bhttp(&mut std::io::Cursor::new(&response_buf[..]))?;

    // let mut output: Box<dyn io::Write> = if let Some(outfile) = &args.output {
    //     Box::new(File::open(outfile)?)
    // } else {
    //     Box::new(std::io::stdout())
    // };
    // if args.binary {
    //     response.write_bhttp(args.mode(), &mut output)?;
    // } else {
    //     response.write_http(&mut output)?;
    // }
    // Ok(())
}
